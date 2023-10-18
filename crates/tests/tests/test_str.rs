#[cfg(feature = "test")]
use anyhow::Result;

use musli::{Decode, Encode};

#[derive(Debug, PartialEq, Encode, Decode)]
struct StructWithStr<'a> {
    name: &'a str,
    age: u32,
}

#[test]
#[cfg(feature = "test")]
fn test_deserialize_roundtrip() -> Result<()> {
    let data = tests::wire::to_vec(&StructWithStr {
        name: "Jane Doe",
        age: 42,
    })?;

    let with_str: StructWithStr<'_> = tests::wire::decode(data.as_slice())?;
    assert_eq!(with_str.name, "Jane Doe");
    assert_eq!(with_str.age, 42);
    Ok(())
}