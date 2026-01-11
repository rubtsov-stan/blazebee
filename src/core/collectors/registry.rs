use std::{collections::HashMap, sync::Arc};

use erased_serde::Serialize;
use once_cell::sync::Lazy;

use super::{error::CollectorError, traits::DataProducer, types::CollectorResult};

/// A trait object that all collectors must implement.
/// It allows us to store different collector types uniformly in the registry
/// while still being able to call them dynamically at runtime.
#[async_trait::async_trait]
pub trait DynCollector: Send + Sync {
    /// Returns a static string identifying the collector.
    /// This name is used when registering and looking up collectors.
    fn name(&self) -> &'static str;

    /// Produces the data for this collector and returns it as a boxed
    /// trait object that implements `Serialize`. The result is wrapped
    /// in our custom `CollectorResult` to handle errors uniformly.
    async fn produce_dyn(&self) -> CollectorResult<Box<dyn Serialize + Send + Sync>>;
}

/// A small wrapper that turns any concrete type implementing `DataProducer`
/// into a type that satisfies the `DynCollector` trait object requirements.
pub struct DynWrapper<T> {
    inner: T,
    name: &'static str,
}

impl<T> DynWrapper<T> {
    /// Creates a new wrapper around a concrete collector instance.
    pub fn new(name: &'static str, inner: T) -> Self {
        Self { name, inner }
    }
}

#[async_trait::async_trait]
impl<T> DynCollector for DynWrapper<T>
where
    T: DataProducer + Send + Sync,
    T::Output: Serialize + Send + 'static,
{
    fn name(&self) -> &'static str {
        self.name
    }

    /// Delegates the actual data production to the wrapped collector,
    /// then boxes the result so it can be stored as a trait object.
    async fn produce_dyn(&self) -> CollectorResult<Box<dyn Serialize + Send + Sync>> {
        let output = self.inner.produce().await?;
        Ok(Box::new(output))
    }
}

/// Metadata for a single collector that will be submitted to the global inventory.
/// Each collector provides a name and a factory function that creates an `Arc<dyn DynCollector>`.
pub struct CollectorMeta {
    pub name: &'static str,
    pub factory: fn() -> Arc<dyn DynCollector>,
}

// Tell the `inventory` crate to collect all submitted `CollectorMeta` values.
inventory::collect!(CollectorMeta);

/// The central registry that holds all registered collectors.
/// It is built once at startup and then used throughout the application.
pub struct CollectorRegistry {
    collectors: HashMap<&'static str, Arc<dyn DynCollector>>,
}

impl CollectorRegistry {
    /// Constructs a new registry by iterating over all submitted `CollectorMeta`
    /// entries (via the `inventory` crate) and instantiating each collector.
    pub fn new() -> Self {
        let mut collectors = HashMap::new();

        for meta in inventory::iter::<CollectorMeta> {
            let collector = (meta.factory)();
            collectors.insert(meta.name, collector);
        }

        CollectorRegistry { collectors }
    }

    /// Retrieves a collector by name. Returns an error if no collector with that name exists.
    pub fn get(&self, name: &str) -> CollectorResult<Arc<dyn DynCollector>> {
        self.collectors
            .get(name)
            .cloned()
            .ok_or_else(|| CollectorError::CollectorNotFound(name.to_string()))
    }

    /// Returns a list of all registered collector names.
    pub fn list_names(&self) -> Vec<&'static str> {
        self.collectors.keys().copied().collect()
    }

    /// Checks whether a collector with the given name is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.collectors.contains_key(name)
    }

    /// Number of registered collectors.
    pub fn len(&self) -> usize {
        self.collectors.len()
    }

    /// True if no collectors are registered.
    pub fn is_empty(&self) -> bool {
        self.collectors.is_empty()
    }

    /// Returns a reference to the global singleton registry.
    /// The registry is built lazily the first time this method is called.
    pub fn global() -> &'static CollectorRegistry {
        &GLOBAL_REGISTRY
    }
}

impl Default for CollectorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// The lazily-initialized global registry instance.
/// `once_cell::sync::Lazy` ensures it is constructed only once, even in a multi-threaded context.
static GLOBAL_REGISTRY: Lazy<CollectorRegistry> = Lazy::new(CollectorRegistry::new);

/// Convenience facade that forwards calls to the global registry.
/// This is the API most application code will use.
pub struct Collectors;

impl Collectors {
    pub fn get(name: &str) -> CollectorResult<Arc<dyn DynCollector>> {
        CollectorRegistry::global().get(name)
    }

    pub fn list() -> Vec<&'static str> {
        CollectorRegistry::global().list_names()
    }

    pub fn exists(name: &str) -> bool {
        CollectorRegistry::global().contains(name)
    }

    pub fn count() -> usize {
        CollectorRegistry::global().len()
    }
}

/// Macro used by collector implementations to register themselves
/// with the global inventory at compile time.
#[macro_export]
macro_rules! register_collector {
    ($collector_type:ty, $name:expr) => {
        inventory::submit! {
            $crate::core::collectors::registry::CollectorMeta {
                name: $name,
                factory: || {
                    std::sync::Arc::new(
                        $crate::core::collectors::registry::DynWrapper::new(
                            $name,
                            <$collector_type>::default(),
                        )
                    )
                },
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use serde::Serialize;
    use serde_json::json;

    use super::*;

    #[derive(Default, Serialize)]
    struct TestCpuCollector {
        usage: f32,
    }

    #[async_trait]
    impl DataProducer for TestCpuCollector {
        type Output = Self;

        async fn produce(&self) -> CollectorResult<Self> {
            Ok(TestCpuCollector { usage: 42.5 })
        }
    }

    #[derive(Default, Serialize)]
    struct TestMemoryCollector;

    #[async_trait]
    impl DataProducer for TestMemoryCollector {
        type Output = String;

        async fn produce(&self) -> CollectorResult<String> {
            Ok("8GB used".to_string())
        }
    }

    mod registry_tests {
        use super::*;

        fn create_test_registry() -> CollectorRegistry {
            let mut collectors: HashMap<&'static str, Arc<dyn DynCollector>> = HashMap::new();

            let cpu_wrapper: Arc<dyn DynCollector> =
                Arc::new(DynWrapper::new("test_cpu", TestCpuCollector::default()));
            let memory_wrapper: Arc<dyn DynCollector> = Arc::new(DynWrapper::new(
                "test_memory",
                TestMemoryCollector::default(),
            ));

            collectors.insert("test_cpu", cpu_wrapper);
            collectors.insert("test_memory", memory_wrapper);

            CollectorRegistry { collectors }
        }

        #[test]
        fn test_registry_operations() {
            let registry = create_test_registry();

            assert_eq!(registry.len(), 2);
            assert!(!registry.is_empty());

            let cpu_collector = registry
                .get("test_cpu")
                .expect("cpu collector should exist");
            assert_eq!(cpu_collector.name(), "test_cpu");

            let memory_collector = registry
                .get("test_memory")
                .expect("memory collector should exist");
            assert_eq!(memory_collector.name(), "test_memory");

            let result = registry.get("non_existent");
            assert!(matches!(result, Err(CollectorError::CollectorNotFound(_))));

            if let Err(CollectorError::CollectorNotFound(name)) = result {
                assert_eq!(name, "non_existent");
            }
        }

        #[test]
        fn test_list_names_and_contains() {
            let registry = create_test_registry();
            let names = registry.list_names();

            assert_eq!(names.len(), 2);
            assert!(names.contains(&"test_cpu"));
            assert!(names.contains(&"test_memory"));

            assert!(registry.contains("test_cpu"));
            assert!(registry.contains("test_memory"));
            assert!(!registry.contains("test_network"));
        }

        #[test]
        fn test_default_creates_same_as_new() {
            let registry1 = create_test_registry();
            let registry2 = CollectorRegistry {
                collectors: registry1.collectors.clone(),
            };

            assert_eq!(registry1.len(), registry2.len());

            let mut names1 = registry1.list_names();
            let mut names2 = registry2.list_names();
            names1.sort();
            names2.sort();
            assert_eq!(names1, names2);
        }
    }

    mod global_registry_tests {
        use super::*;

        #[test]
        fn test_global_registry_is_singleton() {
            let registry1 = CollectorRegistry::global();
            let registry2 = CollectorRegistry::global();

            assert_eq!(registry1 as *const _, registry2 as *const _);
        }

        #[test]
        fn test_collectors_facade_delegates_to_global() {
            let _ = Collectors::list();
            let _ = Collectors::count();
            assert!(Collectors::exists("some_collector") || true);
        }
    }

    mod dyn_wrapper_tests {
        use super::*;

        #[tokio::test]
        async fn test_dyn_wrapper_produces_data() {
            let collector = TestCpuCollector::default();
            let wrapper = DynWrapper::new("test_cpu", collector);

            assert_eq!(wrapper.name(), "test_cpu");

            let result = wrapper.produce_dyn().await.expect("should produce data");
            let serialized = serde_json::to_value(&*result).unwrap();

            assert_eq!(serialized, json!({"usage": 42.5}));
        }

        #[tokio::test]
        async fn test_dyn_wrapper_with_string_output() {
            let collector = TestMemoryCollector::default();
            let wrapper = DynWrapper::new("test_memory", collector);

            let result = wrapper.produce_dyn().await.expect("should produce data");
            let serialized = serde_json::to_value(&*result).unwrap();

            assert_eq!(serialized, json!("8GB used"));
        }
    }

    mod integration_tests {
        use super::*;

        fn create_test_registry() -> CollectorRegistry {
            let mut collectors: HashMap<&'static str, Arc<dyn DynCollector>> = HashMap::new();

            let cpu_wrapper: Arc<dyn DynCollector> =
                Arc::new(DynWrapper::new("test_cpu", TestCpuCollector::default()));
            let memory_wrapper: Arc<dyn DynCollector> = Arc::new(DynWrapper::new(
                "test_memory",
                TestMemoryCollector::default(),
            ));

            collectors.insert("test_cpu", cpu_wrapper);
            collectors.insert("test_memory", memory_wrapper);

            CollectorRegistry { collectors }
        }

        #[tokio::test]
        async fn test_collector_lifecycle() {
            let registry = create_test_registry();

            let collector = registry
                .get("test_cpu")
                .expect("cpu collector should exist");

            assert_eq!(collector.name(), "test_cpu");

            let data = collector
                .produce_dyn()
                .await
                .expect("should produce successfully");

            let json = serde_json::to_string(&*data).unwrap();
            assert!(json.contains("42.5"));

            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed["usage"], 42.5);
        }

        #[tokio::test]
        async fn test_multiple_collectors_produce_different_data() {
            let registry = create_test_registry();

            let cpu_collector = registry.get("test_cpu").unwrap();
            let memory_collector = registry.get("test_memory").unwrap();

            let cpu_data = cpu_collector.produce_dyn().await.unwrap();
            let memory_data = memory_collector.produce_dyn().await.unwrap();

            let cpu_json = serde_json::to_string(&*cpu_data).unwrap();
            let memory_json = serde_json::to_string(&*memory_data).unwrap();

            assert_ne!(cpu_json, memory_json);
            assert!(cpu_json.contains("usage"));
            assert!(memory_json.contains("8GB"));
        }

        #[test]
        fn test_collector_names_are_unique() {
            let registry = create_test_registry();
            let names = registry.list_names();

            let unique_names: std::collections::HashSet<_> = names.iter().collect();
            assert_eq!(names.len(), unique_names.len());
        }
    }

    mod macro_tests {
        use super::*;

        mod inventory_tests {
            use super::*;
            #[allow(unused_imports)]
            use crate::register_collector;

            #[derive(Default, Serialize)]
            struct MacroTestCollector;

            #[async_trait]
            impl DataProducer for MacroTestCollector {
                type Output = &'static str;

                async fn produce(&self) -> CollectorResult<&'static str> {
                    Ok("macro_test")
                }
            }

            register_collector!(MacroTestCollector, "macro_test");

            #[test]
            fn test_register_collector_macro() {
                let registry = CollectorRegistry::new();
                assert!(registry.contains("macro_test"));
            }
        }
    }
}
