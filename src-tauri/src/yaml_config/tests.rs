use super::*;

#[test]
fn removing_merge_keys_in_flow_and_block_maps_preserves_alias_definitions_and_other_entries() {
    for mapping in [
        "{<<: *base, explicit: keep}",
        "{explicit: keep, <<: *base}",
        "{first: keep, <<: *base, last: keep}",
        "\n  <<: *base\n  explicit: keep",
        "\n  explicit: keep\n  <<: *base",
    ] {
        let source = format!(
            "\u{feff}# head\r\nbase: &base {{inherited: keep}}\r\nselected: {}\r\n# tail\r\n",
            mapping.replace('\n', "\r\n")
        );
        let output = edit_yaml_text(&source, |root| {
            root.get_mut(Value::String("selected".into()))
                .unwrap()
                .as_mapping_mut()
                .unwrap()
                .remove(Value::String("<<".into()));
            Ok(())
        })
        .unwrap();
        assert!(output.starts_with('\u{feff}'));
        assert!(output.contains("base: &base {inherited: keep}"));
        assert!(output.contains("# head"));
        assert!(output.contains("# tail"));
        assert!(!output.replace("\r\n", "").contains('\n'));
        let parsed: Value = serde_yaml::from_str(output.trim_start_matches('\u{feff}')).unwrap();
        assert!(parsed["selected"].get("<<").is_none());
        for name in ["explicit", "first", "last"] {
            if mapping.contains(name) {
                assert_eq!(parsed["selected"][name], "keep");
            }
        }
    }
}

#[test]
fn unchanged_merge_keys_retain_exact_syntax_and_changed_merge_values_are_selected_edits() {
    let source =
        "# head\nbase: &base {inherited: keep}\nselected: {<<: *base, explicit: keep}\n# tail\n";
    assert_eq!(edit_yaml_text(source, |_| Ok(())).unwrap(), source);
    let output = edit_yaml_text(source, |root| {
        let entries = root
            .get_mut(Value::String("selected".into()))
            .unwrap()
            .as_mapping_mut()
            .unwrap();
        let mut inherited = Mapping::new();
        inherited.insert(
            Value::String("inherited".into()),
            Value::String("updated".into()),
        );
        entries.insert(Value::String("<<".into()), Value::Mapping(inherited));
        Ok(())
    })
    .unwrap();
    assert!(output.contains("base: &base {inherited: keep}"));
    let mut effective: Value = serde_yaml::from_str(&output).unwrap();
    effective.apply_merge().unwrap();
    assert_eq!(effective["selected"]["inherited"], "updated");
    assert_eq!(effective["selected"]["explicit"], "keep");
}

#[test]
fn changes_to_an_anchored_value_refuse_when_they_would_mutate_an_unselected_alias() {
    let source = "selected: &shared {command: node}\nunselected: *shared\n";
    let result = edit_yaml_text(source, |root| {
        root.get_mut(Value::String("selected".into()))
            .unwrap()
            .as_mapping_mut()
            .unwrap()
            .insert(
                Value::String("command".into()),
                Value::String("other".into()),
            );
        Ok(())
    });
    assert!(result.is_err());
}
