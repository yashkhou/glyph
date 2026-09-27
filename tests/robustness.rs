use glyph::{Value, parse_scene, set_source_property};

const BASE: &str = r##"scene "Fuzz" {
  canvas 320 240 background "#ffffff";
  rect box @id("box") { x 20px; y 30px; width 100px; height 80px; fill "#123456"; }
  text label @id("label") { content "Hello"; x 40px; y 60px; size 20px; }
  constrain box.left >= 0px @required;
}"##;

#[test]
fn parser_survives_deterministic_mutation_corpus_without_panicking() {
    let replacements = [b' ', b';', b'{', b'0', b'"', b'/'];
    let bytes = BASE.as_bytes();
    let mut cases = 0usize;
    for index in 0..bytes.len() {
        if !bytes[index].is_ascii() {
            continue;
        }
        for replacement in replacements {
            let mut mutated = bytes.to_vec();
            mutated[index] = replacement;
            let candidate = String::from_utf8(mutated).unwrap();
            let result = std::panic::catch_unwind(|| parse_scene(&candidate, "mutation.glyph"));
            assert!(result.is_ok(), "parser panicked at byte {index}");
            cases += 1;
        }
    }
    assert!(cases > 1_000);
}

#[test]
fn malformed_source_edits_fail_closed_without_partial_output() {
    let malformed = "scene \"Broken\" { canvas 100 100; rect box { x 0;";
    let result = std::panic::catch_unwind(|| {
        set_source_property(malformed, "box", "x", Value::Number(10.0))
    });
    assert!(result.is_ok());
    assert!(result.unwrap().is_err());

    let valid = "scene \"Valid\" { canvas 100 100; rect box @id(\"box\") { x 0; y 0; width 10; height 10; } }";
    for property in ["x", "width", "opacity", "rotate"] {
        let value = if property == "opacity" {
            Value::Number(2.0)
        } else {
            Value::Text("not-a-number".into())
        };
        let result =
            std::panic::catch_unwind(|| set_source_property(valid, "box", property, value.clone()));
        assert!(result.is_ok(), "source edit panicked for {property}");
        if property != "opacity" {
            assert!(result.unwrap().is_err());
        }
    }
}
