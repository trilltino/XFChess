//! Expected score of the stronger player, in ten-thousandths, for a rating gap of 0, 10, 20 ...
//! 800 points: `10000 / (1 + 10^(-gap / 400))`, rounded. A lookup with linear interpolation
//! replaces `powf`, which is slow on-chain and not bit-exact across targets.
//!
//! Regenerate with: `[round(10000 / (1 + 10 ** (-d / 400))) for d in range(0, 801, 10)]`.
pub const STEP_POINTS: u32 = 10;
pub const MAX_GAP_POINTS: u32 = 800;

pub const EXPECTED_STRONG: [u16; 81] = [
    5000, 5144, 5288, 5431, 5573, 5715, 5855, 5994, 6131,
    6267, 6401, 6532, 6661, 6788, 6912, 7034, 7153, 7268,
    7381, 7491, 7597, 7701, 7801, 7898, 7992, 8083, 8171,
    8255, 8337, 8415, 8490, 8563, 8632, 8698, 8762, 8823,
    8882, 8938, 8991, 9042, 9091, 9137, 9182, 9224, 9264,
    9302, 9339, 9374, 9406, 9438, 9468, 9496, 9523, 9548,
    9572, 9595, 9617, 9638, 9657, 9676, 9693, 9710, 9726,
    9741, 9755, 9768, 9781, 9793, 9804, 9815, 9825, 9835,
    9844, 9853, 9861, 9868, 9876, 9883, 9889, 9895, 9901,
];
