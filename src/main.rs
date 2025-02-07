use validator::Validate;
use raw_struct_macro::RawStruct;

#[derive(RawStruct)]
pub struct Record {
    pub a: Option<i32>,
    pub b: Option<i32>,
    pub c: Option<String>,
    pub d: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let csv_data = "\
a,b,c,d
10,20,foo,bar
aaaa,30,baz,qux
,40,hello,world
";

    let mut rdr = csv::Reader::from_reader(csv_data.as_bytes());

    for result in rdr.deserialize() {
        let raw: RawRecord = result?;
        if let Err(errors) = raw.validate() {
            println!("バリデーションエラー: {:?}", errors);
            return Ok(());
        }
    }
    Ok(())
}