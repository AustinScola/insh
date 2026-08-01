//! Caches values which are expensive to determine.
use std::collections::HashMap;
use std::hash::Hash;

/// Caches values by key so that each value is only determined once.
pub struct Cache<Key, Value> {
    /// The values which have been determined so far by their key.
    values: HashMap<Key, Value>,
}

impl<Key, Value> Cache<Key, Value>
where
    Key: Eq + Hash,
    Value: Clone,
{
    /// Return a new empty cache.
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    /// Return the value for the `key`, determining it with the `determine` function if it is not
    /// cached yet.
    pub fn get<Determine>(&mut self, key: Key, determine: Determine) -> Value
    where
        Determine: FnOnce(&Key) -> Value,
    {
        if let Some(value) = self.values.get(&key) {
            return value.clone();
        }

        let value: Value = determine(&key);
        self.values.insert(key, value.clone());
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_values_are_determined_once_per_key() {
        let mut cache: Cache<u32, String> = Cache::new();
        let mut determined: Vec<u32> = Vec::new();

        for key in [1, 2, 1, 2, 1] {
            let value: String = cache.get(key, |key| {
                determined.push(*key);
                key.to_string()
            });

            assert_eq!(value, key.to_string());
        }

        assert_eq!(determined, vec![1, 2]);
    }
}
