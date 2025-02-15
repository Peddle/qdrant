use collection::operations::point_ops::{PointInsertOperations, PointOperations, PointStruct};
use collection::operations::types::SearchRequest;
use collection::operations::CollectionUpdateOperations;
use segment::types::WithPayloadInterface;
use tokio::runtime::Handle;

use crate::common::{simple_collection_fixture, N_SHARDS};

mod common;

#[tokio::test]
async fn test_collection_paginated_search() {
    test_collection_paginated_search_with_shards(1).await;
    test_collection_paginated_search_with_shards(N_SHARDS).await;
}

async fn test_collection_paginated_search_with_shards(shard_number: u32) {
    let collection_dir = tempdir::TempDir::new("test_collection_paginated_search").unwrap();

    let mut collection = simple_collection_fixture(collection_dir.path(), shard_number).await;

    // Upload 1000 random vectors to the collection
    let mut points = Vec::new();
    for i in 0..1000 {
        points.push(PointStruct {
            id: i.into(),
            vector: vec![i as f32, 0.0, 0.0, 0.0],
            payload: Some(serde_json::from_str(r#"{"number": "John Doe"}"#).unwrap()),
        });
    }
    let insert_points = CollectionUpdateOperations::PointOperation(PointOperations::UpsertPoints(
        PointInsertOperations::PointsList(points),
    ));
    collection
        .update_from_client(insert_points, true)
        .await
        .unwrap();

    let query_vector = vec![1.0, 0.0, 0.0, 0.0];

    let full_search_request = SearchRequest {
        vector: query_vector.clone(),
        filter: None,
        limit: 100,
        offset: 0,
        with_payload: Some(WithPayloadInterface::Bool(true)),
        with_vector: false,
        params: None,
        score_threshold: None,
    };

    let reference_result = collection
        .search(full_search_request, &Handle::current(), None)
        .await
        .unwrap();

    assert_eq!(reference_result.len(), 100);
    assert_eq!(reference_result[0].id, 999.into());

    collection.before_drop().await;

    let page_size = 10;

    let page_1_request = SearchRequest {
        vector: query_vector.clone(),
        filter: None,
        limit: 10,
        offset: page_size,
        with_payload: Some(WithPayloadInterface::Bool(true)),
        with_vector: false,
        params: None,
        score_threshold: None,
    };

    let page_1_result = collection
        .search(page_1_request, &Handle::current(), None)
        .await
        .unwrap();

    // Check that the first page is the same as the reference result
    assert_eq!(page_1_result.len(), 10);
    for i in 0..10 {
        assert_eq!(page_1_result[i], reference_result[page_size + i]);
    }

    let page_9_request = SearchRequest {
        vector: query_vector.clone(),
        filter: None,
        limit: 10,
        offset: page_size * 9,
        with_payload: Some(WithPayloadInterface::Bool(true)),
        with_vector: false,
        params: None,
        score_threshold: None,
    };

    let page_9_result = collection
        .search(page_9_request, &Handle::current(), None)
        .await
        .unwrap();

    // Check that the 9th page is the same as the reference result
    assert_eq!(page_9_result.len(), 10);
    for i in 0..10 {
        assert_eq!(page_9_result[i], reference_result[page_size * 9 + i]);
    }
}
