use super::*;

#[test]
fn reasoning_aliases_deduplicate_equal_values_without_hiding_conflicts() {
    assert_eq!(reasoning(Some(""), Some("real")).unwrap(), Some("real"));
    assert_eq!(reasoning(Some("real"), Some("real")).unwrap(), Some("real"));
    assert_eq!(reasoning(None, Some("")).unwrap(), None);
    assert!(reasoning(Some("one"), Some("two")).is_err());
}

#[test]
fn typed_text_thinking_and_refusals_preserve_their_order_and_unicode() {
    let raw = r#"[{"type":"text","text":"前🦀"},{"type":"thinking","thinking":[{"type":"text","text":"想"},{"type":"text","text":"一下"}],"closed":true},{"type":"output_text","text":"后"},{"type":"thinking","thinking":"再想"},{"type":"refusal","refusal":"无法协助"}]"#;
    assert_eq!(
        parse(raw).unwrap(),
        vec![
            Part {
                kind: Kind::Text,
                text: "前🦀".into()
            },
            Part {
                kind: Kind::Thinking,
                text: "想一下".into()
            },
            Part {
                kind: Kind::Text,
                text: "后".into()
            },
            Part {
                kind: Kind::Thinking,
                text: "再想".into()
            },
            Part {
                kind: Kind::Text,
                text: "无法协助".into()
            },
        ]
    );
    assert_eq!(parse(r#""普通\u6587本""#).unwrap()[0].text, "普通文本");
    assert!(parse("null").unwrap().is_empty());
    assert!(parse("[]").unwrap().is_empty());
}

#[test]
fn unknown_signed_malformed_and_duplicate_parts_fail_without_secret_data() {
    for raw in [
        r#"[{"type":"image_url","image_url":{"url":"secret://image"}}]"#,
        r#"[{"type":"thinking","thinking":"secret-thought","signature":"secret-signature"}]"#,
        r#"[{"type":"text","text":"secret-one","text":"secret-two"}]"#,
        r#"[{"type":"text","type":"thinking","text":"secret"}]"#,
        r#"[{"type":"thinking","thinking":[{"type":"text","text":"a","text":"b"}]}]"#,
        r#"[{"type":"thinking","thinking":[{"type":"image","text":"secret"}]}]"#,
        r#"[{"type":"thinking","thinking":"secret","closed":"secret"}]"#,
        r#"[{"type":"text","text":null}]"#,
        r#"[{"type":"thinking","thinking":{}}]"#,
        "[null]",
        "123",
        "{}",
        "[",
    ] {
        assert_eq!(parse(raw).unwrap_err(), INVALID, "{raw}");
    }
}

#[test]
fn part_counts_include_nested_thinking_and_enforce_byte_boundaries() {
    let boundary = format!(
        "[{}]",
        std::iter::repeat_n(
            r#"{"type":"text","text":""}"#,
            super::super::stream_limits::MAX_BLOCKS
        )
        .collect::<Vec<_>>()
        .join(",")
    );
    assert_eq!(
        parse(&boundary).unwrap().len(),
        super::super::stream_limits::MAX_BLOCKS
    );
    assert_eq!(
        parse(&boundary.replacen('[', r#"[{"type":"text","text":""},"#, 1)).unwrap_err(),
        LIMIT
    );
    let nested = format!(r#"[{{"type":"thinking","thinking":{boundary}}}]"#);
    assert_eq!(parse(&nested).unwrap_err(), LIMIT);
    let max = super::super::stream_frames::MAX_FRAME_BYTES;
    let text = format!("\"{}\"", "x".repeat(max - 2));
    assert_eq!(parse(&text).unwrap()[0].text.len(), max - 2);
    assert_eq!(parse(&(text + " ")).unwrap_err(), LIMIT);
}
