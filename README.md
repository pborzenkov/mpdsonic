# `mpdsonic` - expose MPD library via Subsonic protocol

`mpdsonic` is a Subsonic compatible music streaming server that uses [MPD][mpd] as a library backend.

## Features

  - Artists/Albums browsing by ID3 tags
  - Playlists management
  - Supports MPD libraries over local FS and HTTP(S)

`mpdsonic` has been tested to work with [DSub][dsub] in "Browse by Tags" mode.

## Example

```bash
$ export MPDSONIC_USERNAME=user
$ export MPDSONIC_PASSWORD=password
$ export MPDSONIC_MPD_PASSWORD=mpd-password # optional

$ mpdsonic -a 0.0.0.0:3000 --mpd-address 127.0.0.1:6600 --mpd-library /music
```

## License

Licensed under [MIT license](LICENSE)

[mpd]: https://musicpd.org
[dsub]: https://github.com/daneren2005/Subsonic
