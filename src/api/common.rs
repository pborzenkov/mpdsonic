use super::types::{AlbumID, ArtistID, CoverArtID, Song, SongID};
use mpd_client::{responses, tag::Tag};
use std::{collections::HashMap, str::FromStr};

pub(crate) fn mpd_song_to_subsonic(song: responses::Song) -> Song {
    let artists = song.artists().join(", ");
    let path = song.file_path().display().to_string();

    Song {
        id: SongID::new(&path),
        title: song.title().map(str::to_string),
        album: song.album().map(str::to_string),
        artist: artists.clone(),
        track: get_single_tag(&song.tags, &Tag::Track),
        disc_number: get_single_tag(&song.tags, &Tag::Disc),
        year: get_song_year(&song),
        genre: song.tags.get(&Tag::Genre).map(|v| v.join(", ")),
        cover_art: CoverArtID::new(&path),
        duration: song.duration.map(|v| v.as_secs()),
        path: path.clone(),
        album_id: song.album().map(|album| AlbumID::new(album, &artists)),
        artist_id: ArtistID::new(&artists),
    }
}

pub(crate) fn get_single_tag<T>(tags: &HashMap<Tag, Vec<String>>, tag: &Tag) -> Option<T>
where
    T: FromStr + std::fmt::Debug,
{
    tags.get(tag)
        .and_then(|v| v.first().and_then(|v| v.parse().ok()))
}

pub(crate) fn get_song_year(song: &responses::Song) -> Option<i32> {
    dbg!(get_single_tag::<String>(&song.tags, &Tag::OriginalDate));
    dbg!(get_single_tag::<String>(&song.tags, &Tag::OriginalDate)?
        .split('-')
        .next()
        .and_then(|y| y.parse().ok()))
}
