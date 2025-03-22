use portaudio::PortAudio;

pub fn print_device_list(pa: &PortAudio, filter: Option<&str>) -> Result<(), portaudio::Error> {
    let ds = pa.devices()?;
    for d in ds {
        let (i, d) = d?;
        if let Some(filter) = filter {
            if !d.name.contains(filter) {
                continue;
            }
        }

        let ty = match (d.max_input_channels, d.max_output_channels) {
            (0, 0) => "NUL",
            (0, _) => "OUT",
            (_, 0) => "IN ",
            (_, _) => "   ",
        };
        let name = d.name.replace('\n', " ").replace('\r', "");
        println!(
            "{:2} {} {} (In {}, Out {}, {}Hz)",
            i.0,
            ty,
            &name,
            d.max_input_channels,
            d.max_output_channels,
            d.default_sample_rate as u32,
        );
    }
    Ok(())
}

pub fn find_device(pa: &PortAudio, filter: &str, is_input: bool) -> anyhow::Result<u32> {
    let ds = pa.devices()?;
    for d in ds {
        let (i, d) = d?;
        match (d.max_input_channels, d.max_output_channels, is_input) {
            (0, 0, _) | (0, _, true) | (_, 0, false) => continue,
            (0, _, false) | (_, 0, true) => {}
            (_, _, _) => continue,
        };

        if d.name.contains(filter) {
            return Ok(i.0);
        }
    }

    anyhow::bail!("device not found: {}", filter)
}
