fn main() {
    // embed the app icon and version info into hellocsv.exe (Windows only)
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=assets/hellocsv.ico");
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/hellocsv.ico");
        res.set("ProductName", "Hello CSV");
        res.set("FileDescription", "Hello CSV");
        res.compile().expect("failed to embed Windows resources");
    }
}
