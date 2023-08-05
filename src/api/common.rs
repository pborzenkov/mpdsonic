use super::{
    types::{AlbumID, ArtistID, CoverArtID, Song, SongID},
    Result,
};
use mpd_client::{commands::StickerFind, responses, tag::Tag, Client};
use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};

pub(crate) const STICKER_RATING: &str = "rating";

pub(crate) fn mpd_song_to_subsonic(song: responses::Song, ratings: &HashMap<String, u8>) -> Song {
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
        user_rating: ratings.get(&song.url).cloned(),
    }
}

pub(crate) async fn get_songs_ratings(
    client: &Client,
    songs: &[responses::Song],
) -> Result<HashMap<String, u8>> {
    let dirs = songs
        .iter()
        .filter_map(|s| s.file_path().parent())
        .collect::<HashSet<_>>();
    let dirs = dirs
        .into_iter()
        .map(|d| d.to_string_lossy())
        .collect::<Vec<_>>();

    let ratings = client
        .command_list(
            dirs.iter()
                .map(|s| StickerFind::new(s, STICKER_RATING))
                .collect::<Vec<_>>(),
        )
        .await?;

    Ok(ratings.into_iter().fold(HashMap::new(), |mut acc, mut r| {
        acc.extend(r.value.drain().filter_map(|(k, v)| {
            let v = v.parse::<u8>().ok()?;
            Some((k, v))
        }));
        acc
    }))
}

pub(crate) fn get_single_tag<T>(tags: &HashMap<Tag, Vec<String>>, tag: &Tag) -> Option<T>
where
    T: FromStr + std::fmt::Debug,
{
    tags.get(tag)
        .and_then(|v| v.first().and_then(|v| v.parse().ok()))
}

pub(crate) fn get_song_year(song: &responses::Song) -> Option<i32> {
    get_single_tag::<String>(&song.tags, &Tag::OriginalDate)?
        .split('-')
        .next()
        .and_then(|y| y.parse().ok())
}
