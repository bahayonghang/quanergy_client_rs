use super::*;

#[test]
fn settings_accepts_sample_uppercase_ring_filter_keys() {
    let xml = r#"
        <Settings>
          <RingFilter><Range0>2.5</Range0><Intensity0>7</Intensity0></RingFilter>
        </Settings>
        "#;
    let mut config = PipelineConfig::default();
    config.apply_settings_xml(xml).unwrap();
    assert_eq!(config.ring_filter.min_range[0], 2.5);
    assert_eq!(config.ring_filter.min_intensity[0], 7);
}

#[test]
fn settings_accepts_cpp_lowercase_ring_filter_keys() {
    let xml = r#"
        <Settings>
          <RingFilter><range1>3.5</range1><intensity1>9</intensity1></RingFilter>
        </Settings>
        "#;
    let mut config = PipelineConfig::default();
    config.apply_settings_xml(xml).unwrap();
    assert_eq!(config.ring_filter.min_range[1], 3.5);
    assert_eq!(config.ring_filter.min_intensity[1], 9);
}
