#[test]
fn founding_status_migration_has_its_schema_prerequisite() {
    let persistent_domains = include_str!("../migrations/0008_bh03_persistent_domains.sql");
    let founding_status = include_str!("../migrations/0010_founding_status_revoked.sql");

    assert!(
        persistent_domains.contains("CREATE TABLE founding_applications"),
        "0008 must create the table altered by 0010"
    );
    assert!(
        founding_status.contains("ALTER TABLE founding_applications"),
        "0010 must retain the founding status migration"
    );
}
