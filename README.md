## butter-video

This is a tool for getting Butteraugli and SSIMULACRA metrics for a video.
These metrics are intended for images, and their current binaries, which are
part of libjxl, only support processing images. In order to use it for videos,
this tool exists.

The current implementation of this tool is bad. It's hackish and it's not fast.
It works by reading in each video frame, making a temporary PNG screenshot for each frame,
comparing the PNG screenshots using the butteraugli_main binary from libjxl,
then averaging together the scores.

### Usage

There are two env vars which control where this tool will look for the other executables.

`BUTTERAUGLI_PATH`: The path to the butteraugli binary
`SSIMULACRA_PATH`: The path to the ssimulacra binary

These default to looking in your PATH for `butteraugli` or `ssimulacra`,
or you can set them to wherever your binaries are located.

Then you can run the tool with either:

`butter-video butter raw.y4m encoded.y4m`

Or:

`butter-video ssimulacra raw.y4m encoded.y4m`

### Obtaining the butteraugli and ssimulacra binaries

#### Arch Linux

Install this AUR package: https://aur.archlinux.org/packages/libjxl-metrics-git

#### Other

TODO (Someone please volunteer to fill this in for other operating systems)
