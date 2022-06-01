use std::collections::BTreeMap;
use std::ops::Bound::{Excluded, Included, Unbounded};
use std::sync::Arc;

use atomic_refcell::AtomicRefCell;
use rocksdb::DB;
use serde_json::Value;

use crate::common::rocksdb_operations::db_write_options;
use crate::entry::entry_point::{OperationError, OperationResult};
use crate::index::field_index::histogram::{Histogram, Point};
use crate::index::field_index::stat_tools::estimate_multi_value_selection_cardinality;
use crate::index::field_index::{
    CardinalityEstimation, PayloadBlockCondition, PayloadFieldIndex, PayloadFieldIndexBuilder,
    PrimaryCondition, ValueIndexer,
};
use crate::index::key_encoding::{
    decode_f64_key_ascending, decode_i64_key_ascending, encode_f64_key_ascending,
    encode_i64_key_ascending,
};
use crate::types::{
    FieldCondition, FloatPayloadType, IntPayloadType, PayloadKeyType, PointOffsetType, Range,
};

const HISTOGRAM_BUCKET_SIZE: usize = 100;

pub trait KeyEncoder: Clone {
    fn encode_key(&self, id: PointOffsetType) -> Vec<u8>;
}

impl KeyEncoder for IntPayloadType {
    fn encode_key(&self, id: PointOffsetType) -> Vec<u8> {
        encode_i64_key_ascending(*self, id)
    }
}

impl KeyEncoder for FloatPayloadType {
    fn encode_key(&self, id: PointOffsetType) -> Vec<u8> {
        encode_f64_key_ascending(*self, id)
    }
}

pub trait KeyDecoder {
    fn decode_key(key: &[u8]) -> (PointOffsetType, Self);
}

impl KeyDecoder for IntPayloadType {
    fn decode_key(key: &[u8]) -> (PointOffsetType, Self) {
        decode_i64_key_ascending(key)
    }
}

impl KeyDecoder for FloatPayloadType {
    fn decode_key(key: &[u8]) -> (PointOffsetType, Self) {
        decode_f64_key_ascending(key)
    }
}

pub trait FromRangeValue {
    fn from_range(range_value: f64) -> Self;
}

impl FromRangeValue for FloatPayloadType {
    fn from_range(range_value: f64) -> Self {
        range_value
    }
}

impl FromRangeValue for IntPayloadType {
    fn from_range(range_value: f64) -> Self {
        range_value as Self
    }
}

pub trait ToRangeValue {
    fn to_range(value: Self) -> f64;
}

impl ToRangeValue for FloatPayloadType {
    fn to_range(value: Self) -> f64 {
        value
    }
}

impl ToRangeValue for IntPayloadType {
    fn to_range(value: Self) -> f64 {
        value as f64
    }
}

pub struct NumericIndex<T: KeyEncoder + KeyDecoder + FromRangeValue + Clone> {
    map: BTreeMap<Vec<u8>, u32>,
    db: Arc<AtomicRefCell<DB>>,
    store_cf_name: String,
    histogram: Histogram,
    points_count: usize,
    max_values_per_point: usize,
    point_to_values: Vec<Vec<T>>,
}

impl<T: KeyEncoder + KeyDecoder + FromRangeValue + ToRangeValue + Clone> NumericIndex<T> {
    #[allow(dead_code)] // TODO(gvelo): remove when this index is integrated in `StructPayloadIndex`
    pub fn new(db: Arc<AtomicRefCell<DB>>, store_cf_name: String) -> Self {
        Self {
            map: BTreeMap::new(),
            db,
            store_cf_name,
            histogram: Histogram::new(HISTOGRAM_BUCKET_SIZE),
            points_count: 0,
            max_values_per_point: 0,
            point_to_values: Default::default(),
        }
    }

    fn add_value(&mut self, id: PointOffsetType, value: T) {
        let key = value.encode_key(id);
        let db_ref = self.db.borrow();
        //TODO(gvelo): currently all the methods on the ValueIndexer trait are infallible so we
        // cannot propagate error from here. We need to remove the unwrap() and handle the
        // cf_handle() result when ValueIndexer is refactored to return OperationResult<>
        let cf_handle = db_ref.cf_handle(&self.store_cf_name).unwrap();
        db_ref
            .put_cf_opt(cf_handle, &key, id.to_be_bytes(), &db_write_options())
            .unwrap();
        Self::add_to_map(&mut self.map, &mut self.histogram, key, id);
    }

    pub fn add_many_to_list(&mut self, idx: PointOffsetType, values: impl IntoIterator<Item = T>) {
        if self.point_to_values.len() <= idx as usize {
            self.point_to_values.resize(idx as usize + 1, Vec::new())
        }
        let values: Vec<T> = values.into_iter().collect();
        for value in &values {
            self.add_value(idx, value.clone());
        }
        if !values.is_empty() {
            self.points_count += 1;
            self.max_values_per_point = self.max_values_per_point.max(values.len());
        }
        self.point_to_values[idx as usize] = values;
    }

    pub fn load(&mut self) -> OperationResult<()> {
        let db_ref = self.db.borrow();
        let cf_handle = db_ref.cf_handle(&self.store_cf_name).ok_or_else(|| {
            OperationError::service_error(&format!(
                "Index flush error: column family {} not found",
                self.store_cf_name
            ))
        })?;

        let mut iter = db_ref.raw_iterator_cf(&cf_handle);

        iter.seek_to_first();

        while iter.valid() {
            let key_bytes = iter.key().unwrap().to_owned();
            let value = u32::from_be_bytes(
                iter.value()
                    .ok_or_else(|| OperationError::service_error("cannot take value iteratror"))?
                    .try_into()
                    .map_err(|_| OperationError::service_error("key with incorrect length"))?,
            );
            let key: T = T::decode_key(key_bytes.as_slice()).1;
            self.map.insert(key_bytes, value);

            if self.point_to_values.len() <= value as usize {
                self.point_to_values.resize(value as usize + 1, Vec::new())
            }

            self.point_to_values[value as usize].push(key);

            iter.next();
        }

        Ok(())
    }

    pub fn flush(&self) -> OperationResult<()> {
        let db_ref = self.db.borrow();
        let cf_handle = db_ref.cf_handle(&self.store_cf_name).ok_or_else(|| {
            OperationError::service_error(&format!(
                "Index flush error: column family {} not found",
                self.store_cf_name
            ))
        })?;
        Ok(db_ref.flush_cf(cf_handle)?)
    }

    pub fn remove_point(&mut self, idx: PointOffsetType) -> OperationResult<()> {
        let db_ref = self.db.borrow();

        let cf_handle = db_ref.cf_handle(&self.store_cf_name).ok_or_else(|| {
            OperationError::service_error(&format!(
                "Index flush error: column family {} not found",
                self.store_cf_name
            ))
        })?;

        if self.point_to_values.len() <= idx as usize {
            return Ok(());
        }

        let removed_values = std::mem::take(&mut self.point_to_values[idx as usize]);

        for value in &removed_values {
            let key = value.encode_key(idx);
            self.map.remove(&key);
            db_ref.delete_cf(cf_handle, &key)?;
        }

        Ok(())
    }

    pub fn get_values(&self, idx: PointOffsetType) -> Option<&Vec<T>> {
        self.point_to_values.get(idx as usize)
    }

    fn range_cardinality(&self, range: &Range) -> CardinalityEstimation {
        let lbound = if let Some(lte) = range.lte {
            Included(lte)
        } else if let Some(lt) = range.lt {
            Excluded(lt)
        } else {
            Unbounded
        };

        let gbound = if let Some(gte) = range.gte {
            Included(gte)
        } else if let Some(gt) = range.gt {
            Excluded(gt)
        } else {
            Unbounded
        };

        let histogram_estimation = self.histogram.estimate(gbound, lbound);
        let min_estimation = histogram_estimation.0 / self.max_values_per_point;
        let max_estimation = histogram_estimation.2;

        let total_values = self.map.len();
        let estimation = estimate_multi_value_selection_cardinality(
            self.points_count,
            total_values,
            histogram_estimation.1,
        )
        .round() as usize;

        CardinalityEstimation {
            primary_clauses: vec![],
            min: min_estimation,
            exp: std::cmp::min(max_estimation, std::cmp::max(estimation, min_estimation)),
            max: max_estimation,
        }
    }

    fn add_to_map(
        map: &mut BTreeMap<Vec<u8>, PointOffsetType>,
        histogram: &mut Histogram,
        key: Vec<u8>,
        id: PointOffsetType,
    ) {
        let to_histogram_point = |key| {
            let (decoded_idx, decoded_val) = T::decode_key(key);
            Point {
                val: T::to_range(decoded_val),
                idx: decoded_idx as usize,
            }
        };
        map.insert(key.clone(), id);
        histogram.insert(
            to_histogram_point(&key),
            |x| {
                let key = T::from_range(x.val).encode_key(x.idx as PointOffsetType);
                map.range((Unbounded, Excluded(key)))
                    .next_back()
                    .map(|(key, _)| to_histogram_point(key))
            },
            |x| {
                let key = T::from_range(x.val).encode_key(x.idx as PointOffsetType);
                map.range((Excluded(key), Unbounded))
                    .next()
                    .map(|(key, _)| to_histogram_point(key))
            },
        );
    }
}

impl<T: KeyEncoder + KeyDecoder + FromRangeValue + ToRangeValue + Clone> PayloadFieldIndex
    for NumericIndex<T>
{
    fn load(&mut self) -> OperationResult<()> {
        NumericIndex::load(self)
    }

    fn flush(&self) -> OperationResult<()> {
        NumericIndex::flush(self)
    }

    fn filter(
        &self,
        condition: &FieldCondition,
    ) -> Option<Box<dyn Iterator<Item = PointOffsetType> + '_>> {
        let cond_range = condition.range.as_ref()?;

        let start_bound = match cond_range {
            Range { gt: Some(gt), .. } => {
                let v: T = T::from_range(gt.to_owned());
                Excluded(v.encode_key(u32::MAX))
            }
            Range { gte: Some(gte), .. } => {
                let v: T = T::from_range(gte.to_owned());
                Included(v.encode_key(u32::MIN))
            }
            _ => Unbounded,
        };

        let end_bound = match cond_range {
            Range { lt: Some(lt), .. } => {
                let v: T = T::from_range(lt.to_owned());
                Excluded(v.encode_key(u32::MIN))
            }
            Range { lte: Some(lte), .. } => {
                let v: T = T::from_range(lte.to_owned());
                Included(v.encode_key(u32::MAX))
            }
            _ => Unbounded,
        };

        Some(Box::new(
            self.map.range((start_bound, end_bound)).map(|(_, v)| *v),
        ))
    }

    fn estimate_cardinality(&self, condition: &FieldCondition) -> Option<CardinalityEstimation> {
        condition.range.as_ref().map(|range| {
            let mut cardinality = self.range_cardinality(range);
            cardinality
                .primary_clauses
                .push(PrimaryCondition::Condition(condition.clone()));
            cardinality
        })
    }

    fn payload_blocks(
        &self,
        _threshold: usize,
        _key: PayloadKeyType,
    ) -> Box<dyn Iterator<Item = PayloadBlockCondition> + '_> {
        todo!()
    }

    fn count_indexed_points(&self) -> usize {
        self.map.len()
    }
}

impl PayloadFieldIndexBuilder for NumericIndex<IntPayloadType> {
    fn add(&mut self, id: PointOffsetType, value: &Value) {
        self.add_point(id, value);
    }
}

impl PayloadFieldIndexBuilder for NumericIndex<FloatPayloadType> {
    fn add(&mut self, id: PointOffsetType, value: &Value) {
        self.add_point(id, value);
    }
}

impl ValueIndexer<IntPayloadType> for NumericIndex<IntPayloadType> {
    fn add_many(&mut self, id: PointOffsetType, values: Vec<IntPayloadType>) {
        self.add_many_to_list(id, values)
    }

    fn get_value(&self, value: &Value) -> Option<IntPayloadType> {
        if let Value::Number(num) = value {
            return num.as_i64();
        }
        None
    }

    fn remove_point(&mut self, id: PointOffsetType) {
        // TODO(gvelo): remove unwrap once we have refactored
        // ValueIndexer to return OperationResult
        NumericIndex::remove_point(self, id).unwrap()
    }
}

impl ValueIndexer<FloatPayloadType> for NumericIndex<FloatPayloadType> {
    fn add_many(&mut self, id: PointOffsetType, values: Vec<FloatPayloadType>) {
        self.add_many_to_list(id, values)
    }

    fn get_value(&self, value: &Value) -> Option<FloatPayloadType> {
        if let Value::Number(num) = value {
            return num.as_f64();
        }
        None
    }

    fn remove_point(&mut self, id: PointOffsetType) {
        // TODO(gvelo): remove unwrap once we have refactored
        // ValueIndexer to return OperationResult
        NumericIndex::remove_point(self, id).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use crate::common::rocksdb_operations::db_options;
    use itertools::Itertools;
    use tempdir::TempDir;

    use super::*;

    const CF_NAME: &str = "test_cf";

    #[test]
    fn test_numeric_index_load_from_disk() {
        let tmp_dir = TempDir::new("test_numeric_index_load").unwrap();
        let file_path = tmp_dir.path().join("index");
        let mut db = DB::open_default(file_path).unwrap();
        db.create_cf(CF_NAME, &db_options()).unwrap();
        let db_ref = Arc::new(AtomicRefCell::new(db));
        let mut index: NumericIndex<_> = NumericIndex::new(db_ref.clone(), CF_NAME.to_string());

        let values = vec![
            vec![1.0],
            vec![1.0],
            vec![1.0],
            vec![1.0],
            vec![1.0],
            vec![2.0],
            vec![2.5],
            vec![2.6],
            vec![3.0],
        ];

        values
            .into_iter()
            .enumerate()
            .for_each(|(idx, values)| index.add_many_to_list(idx as PointOffsetType + 1, values));

        index.flush().unwrap();

        let mut new_index: NumericIndex<f64> = NumericIndex::new(db_ref, CF_NAME.to_string());
        new_index.load().unwrap();

        test_cond(
            &new_index,
            Range {
                gt: None,
                gte: None,
                lt: None,
                lte: Some(2.6),
            },
            vec![1, 2, 3, 4, 5, 6, 7, 8],
        );
    }

    #[test]
    fn test_numeric_index() {
        let tmp_dir = TempDir::new("test_numeric_index").unwrap();
        let file_path = tmp_dir.path().join("index");
        let mut db = DB::open_default(file_path).unwrap();
        db.create_cf(CF_NAME, &db_options()).unwrap();
        let db_ref = Arc::new(AtomicRefCell::new(db));

        let values = vec![
            vec![1.0],
            vec![1.0],
            vec![1.0],
            vec![1.0],
            vec![1.0],
            vec![2.0],
            vec![2.5],
            vec![2.6],
            vec![3.0],
        ];

        let mut index: NumericIndex<_> = NumericIndex::new(db_ref, CF_NAME.to_string());

        values
            .into_iter()
            .enumerate()
            .for_each(|(idx, values)| index.add_many_to_list(idx as PointOffsetType + 1, values));

        test_cond(
            &index,
            Range {
                gt: Some(1.0),
                gte: None,
                lt: None,
                lte: None,
            },
            vec![6, 7, 8, 9],
        );

        test_cond(
            &index,
            Range {
                gt: None,
                gte: Some(1.0),
                lt: None,
                lte: None,
            },
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9],
        );

        test_cond(
            &index,
            Range {
                gt: None,
                gte: None,
                lt: Some(2.6),
                lte: None,
            },
            vec![1, 2, 3, 4, 5, 6, 7],
        );

        test_cond(
            &index,
            Range {
                gt: None,
                gte: None,
                lt: None,
                lte: Some(2.6),
            },
            vec![1, 2, 3, 4, 5, 6, 7, 8],
        );

        test_cond(
            &index,
            Range {
                gt: None,
                gte: Some(2.0),
                lt: None,
                lte: Some(2.6),
            },
            vec![6, 7, 8],
        );
    }

    fn test_cond<T: KeyEncoder + KeyDecoder + FromRangeValue + Clone>(
        index: &NumericIndex<T>,
        rng: Range,
        result: Vec<u32>,
    ) {
        let condition = FieldCondition {
            key: "".to_string(),
            r#match: None,
            range: Some(rng),
            geo_bounding_box: None,
            geo_radius: None,
            values_count: None,
        };

        let offsets = index.filter(&condition).unwrap().collect_vec();

        assert_eq!(offsets, result);
    }
}
