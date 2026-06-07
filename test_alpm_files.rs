extern crate alpm;
fn main() {
    let mut handle = alpm::Alpm::new("/", "/var/lib/pacman").unwrap();
    // Try to see if files_update exists
    // handle.files_update(); 
    println!("Alpm handle created");
}
