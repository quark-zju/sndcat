fn main() {
    #[cfg(windows)]
    {
        std::env::set_var("VCPKGRS_DYNAMIC", "1");
        let vcpkg_portaudio = vcpkg::find_package("portaudio").unwrap();
        let mut build = cc::Build::new();
        build.file("csrc/portaudio-wsapi.c").include("csrc");
        for path in vcpkg_portaudio.include_paths {
            build.include(path);
        }
        build.compile("portaudio-wsapi");
    }
}
