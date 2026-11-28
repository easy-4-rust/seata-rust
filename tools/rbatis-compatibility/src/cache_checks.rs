use rbatis::rbdc::common::StatementCache;
#[test]
fn statement_cache_preserves_mutation_replacement_and_lru_resources() {
    let mut cache=StatementCache::new(2);
    assert_eq!(cache.insert("one",1),None);
    assert_eq!(cache.insert("two",2),None);
    *cache.get_mut("one").unwrap()=11;
    assert_eq!(cache.insert("three",3),Some(2));
    assert_eq!(cache.insert("one",12),Some(11));
    assert_eq!(cache.len(),2);
    assert_eq!(cache.remove("three"),Some(3));
    assert_eq!(cache.remove_lru(),Some(12));
    assert_eq!(cache.remove_lru(),None);
    cache.insert("four",4);cache.clear();assert_eq!(cache.len(),0);
    assert_eq!(cache.capacity(),2);
}
