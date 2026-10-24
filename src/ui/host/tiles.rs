//! A question's picture, revealed by zooming out or tile by tile.

use dioxus::prelude::*;

use crate::ui::widgets::QuestionMedia;
use crate::{model::ImageReveal, protocol::QuestionInfo};

/// How close the zoomed picture starts: this many times its size.
const MAX_ZOOM: f64 = 5.0;

/// Columns and rows of tiles covering a picture uncovered tile by tile.
const TILE_COLUMNS: usize = 6;

const TILE_ROWS: usize = 4;

const TILES: usize = TILE_COLUMNS * TILE_ROWS;

/// A question's picture, revealed as the clock runs: `left` is the share of the clock left (1
/// when it starts, 0 at the buzzer) and `seed` varies the order tiles drop away.
#[component]
pub(super) fn QuestionImage(
    image: String,
    reveal: ImageReveal,
    left: f64,
    seed: usize,
    #[props(default)] paused: bool,
) -> Element {
    let left = left.clamp(0.0, 1.0);
    match reveal {
        ImageReveal::Shown => rsx! {
            QuestionMedia { name: image, class: "question-image", autoplay: true, paused }
        },
        ImageReveal::Zoom => {
            let scale = 1.0 + (MAX_ZOOM - 1.0) * left;
            rsx! {
                div { class: "reveal-frame",
                    QuestionMedia {
                        name: image,
                        class: "question-image zoomed",
                        style: "transform: scale({scale:.3})",
                        autoplay: true,
                        paused,
                    }
                }
            }
        }
        ImageReveal::Tiles => {
            let gone = tiles_gone(left);
            let order = tile_order(seed);
            rsx! {
                div { class: "reveal-frame",
                    QuestionMedia { name: image, class: "question-image", autoplay: true, paused }
                    div { class: "tiles", aria_hidden: "true",
                        for (i, rank) in order.into_iter().enumerate() {
                            div { key: "{i}", class: if rank < gone { "tile gone" } else { "tile" } }
                        }
                    }
                }
            }
        }
    }
}

/// How many tiles have dropped away with `left` of the clock to go: an even pace, all gone at
/// the buzzer.
fn tiles_gone(left: f64) -> usize {
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "a handful of tiles, `left` within [0, 1]"
    )]
    let gone = ((1.0 - left) * TILES as f64).floor() as usize;
    gone.min(TILES)
}

/// A picture that was being revealed, shown whole once the question closes (it may close before
/// the reveal completes, when every team has answered).
#[component]
pub(super) fn FullPicture(question: QuestionInfo) -> Element {
    match (&question.image, question.image_reveal) {
        (Some(image), ImageReveal::Zoom | ImageReveal::Tiles) => rsx! {
            QuestionMedia { name: image.clone(), class: "question-image revealed-picture" }
        },
        _ => rsx! {},
    }
}

/// For each tile, its turn to drop away: a shuffle of `0..TILES` that depends on `seed` only.
fn tile_order(seed: usize) -> Vec<usize> {
    let mut order: Vec<usize> = (0..TILES).collect();
    // A small linear congruential generator is plenty to scatter a few tiles.
    let mut state = (seed as u64).wrapping_mul(6_364_136_223_846_793_005) ^ 0x9E37_79B9_7F4A_7C15;
    for i in (1..TILES).rev() {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let j = usize::try_from((state >> 33) % (i as u64 + 1)).unwrap_or(0);
        order.swap(i, j);
    }
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiles_drop_away_at_an_even_pace() {
        assert_eq!(tiles_gone(1.0), 0);
        assert_eq!(tiles_gone(0.5), TILES / 2);
        assert_eq!(tiles_gone(0.0), TILES);
        assert_eq!(tiles_gone(-0.5), TILES);
    }

    #[test]
    fn tile_order_is_a_stable_shuffle_per_question() {
        let order = tile_order(3);
        let mut sorted = order.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..TILES).collect::<Vec<_>>());
        assert_eq!(order, tile_order(3));
        assert_ne!(order, tile_order(4));
        assert_ne!(order, (0..TILES).collect::<Vec<_>>());
    }
}
