//! Prints `docs/field-reference.md` from the schema: `cargo run -p tutti-schema --bin fields >
//! docs/field-reference.md`. A test holds the committed file equal to this output.

fn main() {
    print!("{}", tutti_schema::field_reference(&tutti_schema::schema()));
}
