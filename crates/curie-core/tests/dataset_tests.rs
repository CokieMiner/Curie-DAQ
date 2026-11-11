//! Integration tests for datasets and the saved-dataset registry.

use curie_core::{Dataset, DatasetRegistry, DatasetType};

#[test]
fn test_registry_assigns_ids_and_selects_the_new_dataset() {
    let mut registry = DatasetRegistry::default();
    assert!(registry.selected().is_none());

    for (index, name) in ["run1", "run2", "run3"].into_iter().enumerate() {
        let timestamp = u64::try_from(index).unwrap_or_default();
        let id = registry.add(Dataset::new(name, DatasetType::AcquireTemporal, timestamp));
        assert_eq!(usize::try_from(id).ok(), Some(index + 1), "ids start at 1");
        assert_eq!(
            registry.selected_index(),
            index,
            "a new dataset is selected"
        );
    }
    assert_eq!(registry.datasets().len(), 3_usize);

    registry.select_index(1_usize);
    registry.select_index(9_usize);
    assert_eq!(
        registry.selected_index(),
        1_usize,
        "out-of-range selection is ignored"
    );

    assert!(registry.remove_at(0_usize));
    assert_eq!(
        registry.selected().map(|dataset| dataset.name.as_str()),
        Some("run2"),
        "removing an earlier dataset keeps the selection on the same one"
    );
    assert!(registry.remove_at(1_usize));
    assert_eq!(
        registry.selected().map(|dataset| dataset.name.as_str()),
        Some("run2"),
        "removing the last dataset moves the selection back"
    );
    assert!(!registry.remove_at(5_usize));
    assert!(registry.remove_at(0_usize));
    assert!(registry.selected().is_none());

    let id = registry.add(Dataset::new("run4", DatasetType::SweepMatrix, 0_u64));
    assert_eq!(id, 4_u32, "ids are never reused");
}
