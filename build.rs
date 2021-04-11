fn main() {
    #[cfg(windows)]
    {
        let _vcpkg_portaudio = vcpkg::find_package("portaudio").unwrap();
    }
}
