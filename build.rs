fn main() {
    #[cfg(windows)]
    {
        let mut config = vcpkg::Config::new();
        let vcpkg_root = std::env::current_dir().unwrap().join("vcpkg");
        config.vcpkg_root(vcpkg_root);
        config.find_package("portaudio").unwrap();
    }
}
