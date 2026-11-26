
use seata_sqlparser::{IndexType, LowerCaseLinkHashMap, MetadataError, MetadataLocale, MetadataMapValue};
use std::{error::Error, sync::Arc};

struct Value(Option<LowerCaseLinkHashMap<Value>>);

impl MetadataMapValue for Value {
    fn metadata_equals(&self, _: Option<&Self>) -> Result<bool, MetadataError> {
        if let Some(map) = &self.0 {
            map.clear()?;
        }
        Ok(true)
    }

    fn metadata_hash(&self, _: &mut dyn FnMut(IndexType) -> i32) -> Result<i32, MetadataError> {
        Ok(0)
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let map = LowerCaseLinkHashMap::new(MetadataLocale::new("en"));
    map.put(Some("K".into()), Some(Arc::new(Value(None))))?;
    let removed = map.entry_set().remove(&"k".into(), Some(&Value(Some(map.clone()))))?;
    println!("{removed}\t{}\t{}", map.len()?, map.is_empty()?);
    Ok(())
}
