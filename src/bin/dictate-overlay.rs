//! Recording pill overlay — run beside `dictate --daemon` when ENABLE_OVERLAY=true.

fn main() -> anyhow::Result<()> {
    dictate::overlay_ui::run()
}