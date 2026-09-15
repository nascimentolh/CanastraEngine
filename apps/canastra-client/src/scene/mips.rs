//! Mip chains for decoded textures, so distant surfaces average their texels instead of shimmering.

/// Every level of `rgba` (`width` × `height` RGBA8) down to 1×1, concatenated largest first, and how
/// many levels there are. Each level averages 2×2 blocks of the one above; odd edges repeat.
pub(super) fn chain(width: u32, height: u32, rgba: &[u8]) -> (u32, Vec<u8>) {
    let mut levels = 1;
    let mut all = rgba.to_vec();
    let (mut size, mut above) = ((width as usize, height as usize), rgba.to_vec());
    while size.0 > 1 || size.1 > 1 {
        let (from_width, from_height) = size;
        let (to_width, to_height) = ((from_width / 2).max(1), (from_height / 2).max(1));
        let texel = |x: usize, y: usize, channel: usize| {
            let index = (y.min(from_height - 1) * from_width + x.min(from_width - 1)) * 4 + channel;
            u32::from(above.get(index).copied().unwrap_or_default())
        };
        let mut level = Vec::with_capacity(to_width * to_height * 4);
        for y in 0..to_height {
            for x in 0..to_width {
                for channel in 0..4 {
                    let (left, top) = (x * 2, y * 2);
                    let sum = texel(left, top, channel)
                        + texel(left + 1, top, channel)
                        + texel(left, top + 1, channel)
                        + texel(left + 1, top + 1, channel);
                    level.push(u8::try_from((sum + 2) / 4).unwrap_or(u8::MAX));
                }
            }
        }
        all.extend_from_slice(&level);
        (size, above, levels) = ((to_width, to_height), level, levels + 1);
    }
    (levels, all)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halves_to_one_texel_averaging_blocks() {
        // 2×1: a black and a white texel average to mid gray at 1×1.
        let (levels, all) = chain(2, 1, &[0, 0, 0, 255, 255, 255, 255, 255]);
        assert_eq!(levels, 2);
        assert_eq!(&all[8..], &[128, 128, 128, 255]);

        let (levels, all) = chain(4, 4, &[10; 64]);
        assert_eq!((levels, all.len()), (3, 64 + 16 + 4));
    }
}
