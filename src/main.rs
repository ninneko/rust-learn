use validator::Validate;
use raw_struct_macro::RawStruct;

#[derive(RawStruct)]
pub struct Record {
    pub a: Option<u16>,
    pub b: Option<i32>,
    #[validate(min_length = 5)]
    pub c: String,
    #[validate(min_length = 5)]
    pub d: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let csv_data = "\
a,b,c,d
43434532432432432,20,foo,
1,30,baz,qux
-3232,40,hello,world
";

    let mut rdr = csv::Reader::from_reader(csv_data.as_bytes());

    for result in rdr.deserialize() {
        let raw: RawRecord = result?;
        if let Err(errors) = raw.validate() {
            println!("バリデーションエラー: {:?}", errors);
        }
        println!("======")
    }
    Ok(())
}