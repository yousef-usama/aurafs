/// Table and index constants for DynamoDB Single-Table Design.
pub const TABLE_NAME_DEFAULT: &str = "aurafs-metadata";
pub const GSI1_NAME: &str = "GSI1-DeviceIndex";
pub const GSI2_NAME: &str = "GSI2-TagIndex";

/// Primary Partition Key attribute name.
pub const PK_ATTR: &str = "pk";
/// Primary Sort Key attribute name.
pub const SK_ATTR: &str = "sk";

/// GSI1 Partition Key attribute name.
pub const GSI1_PK_ATTR: &str = "gsi1_pk";
/// GSI1 Sort Key attribute name.
pub const GSI1_SK_ATTR: &str = "gsi1_sk";

/// GSI2 Partition Key attribute name.
pub const GSI2_PK_ATTR: &str = "gsi2_pk";
/// GSI2 Sort Key attribute name.
pub const GSI2_SK_ATTR: &str = "gsi2_sk";

/// Generate the primary Partition Key for a file entity: `FILE#<relative_path>`.
pub fn file_pk(relative_path: &str) -> String {
    format!("FILE#{}", relative_path.trim_start_matches('/'))
}

/// Constant Sort Key for file metadata: `#META`.
pub const FILE_META_SK: &str = "#META";

/// Generate Sort Key for a device-specific sync state item: `#DEVICE#<device_id>`.
pub fn device_state_sk(device_id: &str) -> String {
    format!("#DEVICE#{}", device_id)
}

/// Generate Partition Key for directory listings: `DIR#<parent_dir>`.
pub fn dir_pk(parent_dir: &str) -> String {
    let clean = parent_dir.trim_matches('/');
    if clean.is_empty() {
        "DIR#root".to_string()
    } else {
        format!("DIR#{}", clean)
    }
}

/// Generate Sort Key for a directory child file: `F#<filename>`.
pub fn dir_file_sk(filename: &str) -> String {
    format!("F#{}", filename)
}

/// Generate GSI1 Partition Key for device-scoped queries: `DEVICE#<device_id>`.
pub fn gsi1_device_pk(device_id: &str) -> String {
    format!("DEVICE#{}", device_id)
}

/// Generate GSI2 Partition Key for tag queries: `TAG#<key>=<value>`.
pub fn gsi2_tag_pk(key: &str, value: &str) -> String {
    format!("TAG#{}={}", key, value)
}

/// Generate GSI2 Sort Key pointing back to the file: `FILE#<relative_path>`.
pub fn gsi2_tag_sk(relative_path: &str) -> String {
    file_pk(relative_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_keys() {
        assert_eq!(file_pk("/docs/specs.md"), "FILE#docs/specs.md");
        assert_eq!(file_pk("docs/specs.md"), "FILE#docs/specs.md");
        assert_eq!(FILE_META_SK, "#META");
        assert_eq!(device_state_sk("dev-123"), "#DEVICE#dev-123");
    }

    #[test]
    fn test_dir_keys() {
        assert_eq!(dir_pk(""), "DIR#root");
        assert_eq!(dir_pk("/"), "DIR#root");
        assert_eq!(dir_pk("/photos/2026/"), "DIR#photos/2026");
        assert_eq!(dir_file_sk("hero.png"), "F#hero.png");
    }

    #[test]
    fn test_gsi_keys() {
        assert_eq!(gsi1_device_pk("laptop-01"), "DEVICE#laptop-01");
        assert_eq!(
            gsi2_tag_pk("environment", "production"),
            "TAG#environment=production"
        );
        assert_eq!(gsi2_tag_sk("config.json"), "FILE#config.json");
    }
}
