# Mic Stop Cleanup and v1.0.5 Release Update

## Goal
Fix realtime dictation so pressing Super+R to stop releases microphone access immediately, without creating a new version number.

## Approach
Update the shared audio lifecycle in `src/audio.rs` because all modes use `AudioRecorder`. On stop, the recorder will mark itself inactive, pause the CPAL stream, explicitly drop the stream, explicitly drop the device handle, and clear the chunk sender used by continuous/realtime capture.

## Rationale
The existing stop path pauses the stream and removes the device handle, but it does not explicitly drop all audio-related resources or close the continuous chunk channel. Some PipeWire/CPAL combinations keep microphone access visible until those resources are fully released.

## Testing
Run the Rust test suite with `cargo test`. Verify `Cargo.toml` remains at `1.0.5`.

## Release Handling
Commit the fix on `main`, keep the package version unchanged at `1.0.5`, and move the existing `v1.0.5` tag to the fixed commit. If remote access is available, force-update only the `v1.0.5` tag rather than creating a new version tag.
