mod common;
mod settings;

use std::path::Path;

use clap::Parser;
use collection::collection::Collection;
use segment::segment_constructor::load_segment;
use tokio::runtime::Runtime;

#[derive(Parser, Debug)]
#[clap(version, about)]
struct Args {
    /// Path to the collection
    #[clap(short, long)]
    pub collection: Option<String>,
    /// Path to the segment
    #[clap(short, long)]
    pub segment: Option<String>,
}

fn load_collection(path: &str) -> Result<Collection, String> {
    let rt = Runtime::new().expect("create runtime");
    let collection_path = Path::new(path);
    let collection_name = collection_path
        .file_name()
        .expect("Can't resolve a filename of one of the collection files")
        .to_str()
        .expect("A filename of one of the collection files is not a valid UTF-8")
        .to_string();
    eprintln!("collection_name = {:#?}", collection_name);
    let mut collection = rt.block_on(Collection::load(
        collection_name.clone(),
        &collection_path,
        &collection_path.join("snapshots"),
        Default::default(),
    ));
    rt.block_on(collection.before_drop());
    Ok(collection)
}

fn main() -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "DEBUG");
    env_logger::init();
    let args: Args = Args::parse();

    {
        if let Some(collection_path) = args.collection {
            let collection = load_collection(&collection_path).unwrap();
        }

        if let Some(segment_path) = args.segment {
            let segment = load_segment(&Path::new(&segment_path)).unwrap();
            let id_tracker = segment.id_tracker.borrow();
        }
    }

    Ok(())
}
