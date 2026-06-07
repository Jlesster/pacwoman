extern crate alpm;
fn main() {
    let d = alpm::Dep::from("test");
    println!("{:?}", d);
}
