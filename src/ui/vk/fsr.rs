//! CPU-side constants for the FSR 1.0 passes (port of FsrEasuCon / FsrRcasCon
//! from AMD's ffx_fsr1.h).

/// EASU constants con0..con3 as 16 floats. Input image == viewport here.
pub fn easu_consts(in_w: f32, in_h: f32, out_w: f32, out_h: f32) -> [f32; 16] {
    let rcp_out_x = 1.0 / out_w;
    let rcp_out_y = 1.0 / out_h;
    let rcp_in_x = 1.0 / in_w;
    let rcp_in_y = 1.0 / in_h;
    [
        in_w * rcp_out_x,
        in_h * rcp_out_y,
        0.5 * in_w * rcp_out_x - 0.5,
        0.5 * in_h * rcp_out_y - 0.5,
        rcp_in_x,
        rcp_in_y,
        1.0 * rcp_in_x,
        -1.0 * rcp_in_y,
        -1.0 * rcp_in_x,
        2.0 * rcp_in_y,
        1.0 * rcp_in_x,
        2.0 * rcp_in_y,
        0.0 * rcp_in_x,
        4.0 * rcp_in_y,
        0.0,
        0.0,
    ]
}

/// RCAS sharpness constant: stops -> linear (FsrRcasCon).
pub fn rcas_sharpness(stops: f32) -> f32 {
    (-stops).exp2()
}

/// Default RCAS sharpness in stops (0.2 = AMD's recommended default).
pub const RCAS_STOPS: f32 = 0.2;
