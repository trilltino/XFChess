/// All configurable search parameters in one place.
#[derive(Clone, Debug)]
pub struct SearchParams {
    pub aspiration_base: i32,
    pub aspiration_mul: i32,
    pub qdelta_margin: i32,
    pub rfp_depth: i32,
    pub rfp_base: i32,
    pub rfp_mul: i32,
    pub rfp_improving: i32,
    pub lmr_depth: i32,
    pub lmr_move_start: i32,
    pub lmr_hd: i32,
    pub lmr_quiet_mul: f64,
    pub lmr_quiet_base: f64,
    pub lmp_depth: i32,
    pub lmp_base: i32,
    pub lmp_improving: i32,
    pub lmp_depth_pow: f64,
    pub futility_depth: i32,
    pub futility_base: i32,
    pub futility_mul: i32,
    pub razor_depth: i32,
    pub razor_base: i32,
    pub razor_mul: i32,
    pub razor_improving: i32,
    pub nmp_base: i32,
    pub nmp_mul: i32,
    pub nmp_slope: i32,
    pub see_depth: i32,
    pub see_quiet_margin: i32,
    pub see_nonquiet_margin: i32,
}

impl Default for SearchParams {
    fn default() -> Self {
        Self::sarah_tuned()
    }
}

impl SearchParams {
    /// Sarah's SPSA-tuned defaults (from search.c init_search_params)
    pub const fn sarah_tuned() -> Self {
        Self {
            aspiration_base: 63,
            aspiration_mul: 3,
            qdelta_margin: 287,
            rfp_depth: 4,
            rfp_base: 4,
            rfp_mul: 37,
            rfp_improving: 70,
            lmr_depth: 3,
            lmr_move_start: 3,
            lmr_hd: 16760,
            lmr_quiet_mul: 0.613,
            lmr_quiet_base: 1.225,
            lmp_depth: 4,
            lmp_base: 8,
            lmp_improving: 1,
            lmp_depth_pow: 1.48,
            futility_depth: 2,
            futility_base: 209,
            futility_mul: 209,
            razor_depth: 1,
            razor_base: 248,
            razor_mul: 112,
            razor_improving: 132,
            nmp_base: 11,
            nmp_mul: 626,
            nmp_slope: 320,
            see_depth: 2,
            see_quiet_margin: -76,
            see_nonquiet_margin: -105,
        }
    }
}
