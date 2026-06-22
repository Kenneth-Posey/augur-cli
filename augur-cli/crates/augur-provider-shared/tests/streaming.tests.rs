use augur_domain::domain::string_newtypes::{AccumulatedText, OutputText};
use augur_provider_shared::{SseChunk, drain_complete_sse_lines};

#[test]
fn drain_complete_sse_lines_preserves_remainder() {
    let mut carry = AccumulatedText::from("data: part");

    let lines = drain_complete_sse_lines(&mut carry, SseChunk::from(&b" one\ndata: two\ntr"[..]));

    assert_eq!(
        lines,
        vec![
            OutputText::from("data: part one"),
            OutputText::from("data: two")
        ]
    );
    assert_eq!(&*carry, "tr");
}
