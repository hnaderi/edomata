//! Port of `JEitherSuite.scala`. `JEither[L, R]` maps to `Result<R, L>`;
//! these tests pin the mapping of every `JEither` operation.

fn left(value: &str) -> Result<i32, String> {
    Err(value.to_string())
}

fn right(value: i32) -> Result<i32, String> {
    Ok(value)
}

#[test]
fn left_creates_err() {
    let e = left("error");
    assert!(e.is_err());
    assert_eq!(e.clone().unwrap_err(), "error");
}

#[test]
fn right_creates_ok() {
    let e = right(42);
    assert!(e.is_ok());
    assert_eq!(e.unwrap(), 42);
}

#[test]
#[should_panic]
fn get_right_on_left_panics() {
    let _ = left("error").unwrap();
}

#[test]
#[should_panic]
fn get_left_on_right_panics() {
    let _ = right(42).unwrap_err();
}

#[test]
fn fold_applies_the_correct_function() {
    let fold = |e: Result<i32, String>| e.map_or_else(|l| format!("L:{l}"), |r| format!("R:{r}"));
    assert_eq!(fold(left("err")), "L:err");
    assert_eq!(fold(right(10)), "R:10");
}

#[test]
fn map_transforms_the_right_value() {
    assert_eq!(right(5).map(|x| format!("v={x}")), Ok("v=5".to_string()));
}

#[test]
fn map_on_left_is_identity() {
    let mapped = left("err").map(|x| format!("v={x}"));
    assert!(mapped.is_err());
    assert_eq!(mapped.unwrap_err(), "err");
}
