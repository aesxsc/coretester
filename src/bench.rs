use std::collections::BTreeMap;
use std::hint::black_box;
use std::time::{Duration, Instant};

pub struct MainSample {
    pub integer_gops: f64,
    pub float_gflops: f64,
    pub memory_gbps: f64,
    pub latency_ns: f64,
    pub stability: f64,
}

fn timed_chunks<F>(duration: Duration, mut chunk: F) -> (u64, Duration)
where
    F: FnMut() -> u64,
{
    let start = Instant::now();
    let mut units = 0u64;
    while start.elapsed() < duration {
        units = units.saturating_add(chunk());
    }
    (units, start.elapsed())
}

fn integer_rate(duration: Duration) -> f64 {
    let mut a = black_box(0x9e3779b97f4a7c15u64);
    let mut b = black_box(0xd1b54a32d192ed03u64);
    let (ops, elapsed) = timed_chunks(duration, || {
        for _ in 0..256 {
            a = a.wrapping_mul(0xd6e8feb86659fd93).rotate_left(17) ^ b;
            b = b.wrapping_add(a ^ 0xa0761d6478bd642f).rotate_right(11);
        }
        black_box(a ^ b);
        256 * 6
    });
    black_box((a, b));
    ops as f64 / elapsed.as_secs_f64() / 1e9
}

fn float_rate(duration: Duration) -> f64 {
    let mut v = black_box([
        1.0000001f64,
        0.9999997,
        1.0000003,
        0.9999991,
        1.0000011,
        0.9999989,
        1.0000007,
        0.9999993,
    ]);
    let (ops, elapsed) = timed_chunks(duration, || {
        for _ in 0..256 {
            v[0] = v[0].mul_add(1.00000001, 0.0000001);
            v[1] = v[1].mul_add(0.99999999, 0.0000002);
            v[2] = v[2].mul_add(1.00000002, -0.0000001);
            v[3] = v[3].mul_add(0.99999998, 0.0000003);
            v[4] = v[4].mul_add(1.00000003, -0.0000002);
            v[5] = v[5].mul_add(0.99999997, 0.0000004);
            v[6] = v[6].mul_add(1.00000004, -0.0000003);
            v[7] = v[7].mul_add(0.99999996, 0.0000005);
        }
        black_box(v);
        256 * 8 * 2
    });
    black_box(v);
    ops as f64 / elapsed.as_secs_f64() / 1e9
}

fn memory_rate(duration: Duration) -> f64 {
    // Larger than typical private L2, small enough not to dominate total runtime.
    let mut data = vec![0x9e3779b97f4a7c15u64; 2 * 1024 * 1024];
    let mut salt = black_box(1u64);
    let bytes_per_pass = data.len() as u64 * 16; // one 8-byte read + one 8-byte write
    let (bytes, elapsed) = timed_chunks(duration, || {
        for item in &mut data {
            *item = item.wrapping_add(salt).rotate_left(7);
        }
        salt = salt.wrapping_add(data[black_box((salt as usize) & (data.len() - 1))]);
        black_box(&data);
        bytes_per_pass
    });
    black_box((data, salt));
    bytes as f64 / elapsed.as_secs_f64() / 1e9
}

fn latency(duration: Duration) -> f64 {
    const N: usize = 512 * 1024; // 4 MiB pointer set on 64-bit targets
    let mut order: Vec<usize> = (0..N).collect();
    let mut rng = 0x243f6a8885a308d3u64;
    for i in (1..N).rev() {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        order.swap(i, rng as usize % (i + 1));
    }
    let mut links = vec![0usize; N];
    for i in 0..N {
        links[order[i]] = order[(i + 1) % N];
    }
    let mut cursor = order[0];
    let (loads, elapsed) = timed_chunks(duration, || {
        for _ in 0..65_536 {
            cursor = links[black_box(cursor)];
        }
        black_box(cursor);
        65_536
    });
    black_box((links, cursor));
    elapsed.as_secs_f64() * 1e9 / loads as f64
}

pub fn run_main(duration: Duration) -> MainSample {
    MainSample {
        integer_gops: integer_rate(duration),
        float_gflops: float_rate(duration),
        memory_gbps: memory_rate(duration),
        latency_ns: latency(duration),
        stability: 0.0,
    }
}

pub fn stability_probe(duration: Duration) -> f64 {
    let slice = (duration / 4).max(Duration::from_millis(3));
    let samples = [
        integer_rate(slice),
        integer_rate(slice),
        integer_rate(slice),
        integer_rate(slice),
    ];
    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    let sd =
        (samples.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / samples.len() as f64).sqrt();
    (100.0 - sd / mean.max(1e-12) * 100.0).clamp(0.0, 100.0)
}

pub fn detected_features() -> Vec<String> {
    let mut f = Vec::new();
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        macro_rules! add {
            ($test:expr, $name:literal) => {
                if $test {
                    f.push($name.into());
                }
            };
        }
        add!(std::is_x86_feature_detected!("sse"), "SSE");
        add!(std::is_x86_feature_detected!("sse2"), "SSE2");
        add!(std::is_x86_feature_detected!("sse3"), "SSE3");
        add!(std::is_x86_feature_detected!("ssse3"), "SSSE3");
        add!(std::is_x86_feature_detected!("sse4.1"), "SSE4.1");
        add!(std::is_x86_feature_detected!("sse4.2"), "SSE4.2");
        add!(std::is_x86_feature_detected!("avx"), "AVX");
        add!(std::is_x86_feature_detected!("avx2"), "AVX2");
        add!(std::is_x86_feature_detected!("fma"), "FMA");
        add!(std::is_x86_feature_detected!("aes"), "AES");
        add!(std::is_x86_feature_detected!("pclmulqdq"), "PCLMULQDQ");
        add!(std::is_x86_feature_detected!("popcnt"), "POPCNT");
        add!(std::is_x86_feature_detected!("bmi1"), "BMI1");
        add!(std::is_x86_feature_detected!("bmi2"), "BMI2");
        add!(std::is_x86_feature_detected!("adx"), "ADX");
        add!(std::is_x86_feature_detected!("sha"), "SHA");
        add!(std::is_x86_feature_detected!("rdrand"), "RDRAND");
        add!(std::is_x86_feature_detected!("rdseed"), "RDSEED");
        add!(std::is_x86_feature_detected!("avx512f"), "AVX512F");
        add!(std::is_x86_feature_detected!("avx512bw"), "AVX512BW");
        add!(std::is_x86_feature_detected!("avx512cd"), "AVX512CD");
        add!(std::is_x86_feature_detected!("avx512dq"), "AVX512DQ");
        add!(std::is_x86_feature_detected!("avx512vl"), "AVX512VL");
        add!(std::is_x86_feature_detected!("gfni"), "GFNI");
        add!(std::is_x86_feature_detected!("vaes"), "VAES");
        add!(std::is_x86_feature_detected!("vpclmulqdq"), "VPCLMULQDQ");
    }
    #[cfg(target_arch = "aarch64")]
    {
        macro_rules! add {
            ($test:expr, $name:literal) => {
                if $test {
                    f.push($name.into());
                }
            };
        }
        add!(std::arch::is_aarch64_feature_detected!("neon"), "NEON");
        add!(std::arch::is_aarch64_feature_detected!("aes"), "AES");
        add!(std::arch::is_aarch64_feature_detected!("pmull"), "PMULL");
        add!(std::arch::is_aarch64_feature_detected!("sha2"), "SHA2");
        add!(std::arch::is_aarch64_feature_detected!("sha3"), "SHA3");
        add!(std::arch::is_aarch64_feature_detected!("crc"), "CRC");
        add!(
            std::arch::is_aarch64_feature_detected!("dotprod"),
            "DOTPROD"
        );
        add!(std::arch::is_aarch64_feature_detected!("fp16"), "FP16");
        add!(std::arch::is_aarch64_feature_detected!("sve"), "SVE");
    }
    if f.is_empty() {
        f.push("baseline".into());
    }
    f
}

pub fn benchmarked_features() -> Vec<String> {
    let mut f = Vec::new();
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if std::is_x86_feature_detected!("sse2") {
            f.push("SSE2 FP".into());
        }
        if std::is_x86_feature_detected!("avx2") {
            f.push("AVX2 integer".into());
        }
        if std::is_x86_feature_detected!("fma") {
            f.push("FMA".into());
        }
        if std::is_x86_feature_detected!("aes") {
            f.push("AES-NI".into());
        }
        if std::is_x86_feature_detected!("sse4.2") {
            f.push("SSE4.2 CRC".into());
        }
        if std::is_x86_feature_detected!("popcnt") {
            f.push("POPCNT".into());
        }
        if std::is_x86_feature_detected!("bmi2") {
            f.push("BMI2 PDEP".into());
        }
        if std::is_x86_feature_detected!("bmi1") {
            f.push("BMI1 bit ops".into());
        }
        if std::is_x86_feature_detected!("pclmulqdq") {
            f.push("PCLMULQDQ".into());
        }
        if std::is_x86_feature_detected!("sha") {
            f.push("SHA-NI".into());
        }
        if std::is_x86_feature_detected!("adx") {
            f.push("ADX carry".into());
        }
        if std::is_x86_feature_detected!("rdrand") {
            f.push("RDRAND".into());
        }
        if std::is_x86_feature_detected!("rdseed") {
            f.push("RDSEED".into());
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        if std::arch::is_aarch64_feature_detected!("neon") {
            f.push("NEON FP".into());
        }
    }
    f
}

pub fn core_kind(_cpu: usize) -> String {
    #[cfg(target_arch = "x86_64")]
    {
        let max = std::arch::x86_64::__cpuid(0).eax;
        if max >= 0x1a {
            let kind = std::arch::x86_64::__cpuid_count(0x1a, 0).eax >> 24;
            return match kind {
                0x20 => "Efficiency".into(),
                0x40 => "Performance".into(),
                0 => "Uniform/unknown".into(),
                x => format!("Hybrid type 0x{x:02x}"),
            };
        }
    }
    #[cfg(target_arch = "x86")]
    {
        let max = std::arch::x86::__cpuid(0).eax;
        if max >= 0x1a {
            let kind = std::arch::x86::__cpuid_count(0x1a, 0).eax >> 24;
            return match kind {
                0x20 => "Efficiency".into(),
                0x40 => "Performance".into(),
                0 => "Uniform/unknown".into(),
                x => format!("Hybrid type 0x{x:02x}"),
            };
        }
    }
    "Uniform/unknown".into()
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
mod x86kernels {
    use super::{black_box, timed_chunks};
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;
    use std::time::Duration;

    #[target_feature(enable = "sse2")]
    pub unsafe fn sse2(duration: Duration) -> f64 {
        let mut a = _mm_set1_pd(black_box(1.0000001));
        let b = _mm_set1_pd(1.00000001);
        let c = _mm_set1_pd(0.00000001);
        let (ops, e) = timed_chunks(duration, || {
            for _ in 0..512 {
                a = _mm_add_pd(_mm_mul_pd(a, b), c);
            }
            black_box(a);
            512 * 2 * 2
        });
        black_box(a);
        ops as f64 / e.as_secs_f64() / 1e9
    }
    #[target_feature(enable = "avx2")]
    pub unsafe fn avx2(duration: Duration) -> f64 {
        let mut a = _mm256_set1_epi32(black_box(3));
        let b = _mm256_set1_epi32(1664525);
        let c = _mm256_set1_epi32(1013904223);
        let (ops, e) = timed_chunks(duration, || {
            for _ in 0..512 {
                a = _mm256_add_epi32(_mm256_mullo_epi32(a, b), c);
            }
            black_box(a);
            512 * 8 * 2
        });
        black_box(a);
        ops as f64 / e.as_secs_f64() / 1e9
    }
    #[target_feature(enable = "avx2,fma")]
    pub unsafe fn fma(duration: Duration) -> f64 {
        let mut a = _mm256_set1_ps(black_box(1.000001));
        let b = _mm256_set1_ps(1.0000001);
        let c = _mm256_set1_ps(0.000001);
        let (ops, e) = timed_chunks(duration, || {
            for _ in 0..512 {
                a = _mm256_fmadd_ps(a, b, c);
            }
            black_box(a);
            512 * 8 * 2
        });
        black_box(a);
        ops as f64 / e.as_secs_f64() / 1e9
    }
    #[target_feature(enable = "aes")]
    pub unsafe fn aes(duration: Duration) -> f64 {
        let mut a = _mm_set1_epi64x(black_box(0x12345678));
        let k = _mm_set1_epi64x(0x0f1e2d3c);
        let (blocks, e) = timed_chunks(duration, || {
            for _ in 0..1024 {
                a = _mm_aesenc_si128(a, k);
            }
            black_box(a);
            1024
        });
        black_box(a);
        blocks as f64 / e.as_secs_f64() / 1e9
    }
    #[target_feature(enable = "sse4.2")]
    pub unsafe fn crc(duration: Duration) -> f64 {
        let mut crc = black_box(0x12345678u64);
        let mut v = black_box(0x9e3779b97f4a7c15u64);
        let (bytes, e) = timed_chunks(duration, || {
            for _ in 0..2048 {
                crc = _mm_crc32_u64(crc, v);
                v = v.wrapping_add(crc);
            }
            black_box(crc);
            2048 * 8
        });
        black_box((crc, v));
        bytes as f64 / e.as_secs_f64() / 1e9
    }
    #[target_feature(enable = "popcnt")]
    pub unsafe fn popcnt(duration: Duration) -> f64 {
        let mut v = black_box(0x9e3779b97f4a7c15u64);
        let mut sum = 0i64;
        let (ops, e) = timed_chunks(duration, || {
            for _ in 0..2048 {
                sum = sum.wrapping_add(_popcnt64(v as i64) as i64);
                v = v.rotate_left(7).wrapping_add(sum as u64);
            }
            black_box(sum);
            2048
        });
        black_box((v, sum));
        ops as f64 / e.as_secs_f64() / 1e9
    }
    #[target_feature(enable = "bmi2")]
    pub unsafe fn bmi2(duration: Duration) -> f64 {
        let mut v = black_box(0x123456789abcdef0u64);
        let mask = 0x5555aaaa5555aaaau64;
        let (ops, e) = timed_chunks(duration, || {
            for _ in 0..2048 {
                v = _pdep_u64(v, mask).rotate_left(1) ^ 0x9e3779b97f4a7c15;
            }
            black_box(v);
            2048
        });
        black_box(v);
        ops as f64 / e.as_secs_f64() / 1e9
    }

    #[target_feature(enable = "bmi1")]
    pub unsafe fn bmi1(duration: Duration) -> f64 {
        let mut v = black_box(0xfedcba9876543210u64);
        let (ops, e) = timed_chunks(duration, || {
            for _ in 0..2048 {
                v = _blsr_u64(v | 0x0101010101010101).rotate_left(3);
            }
            black_box(v);
            2048
        });
        black_box(v);
        ops as f64 / e.as_secs_f64() / 1e9
    }

    #[target_feature(enable = "pclmulqdq")]
    pub unsafe fn pclmul(duration: Duration) -> f64 {
        let mut a = _mm_set1_epi64x(black_box(0x0123456789abcdefu64) as i64);
        let b = _mm_set1_epi64x(0x1d872b41);
        let (ops, e) = timed_chunks(duration, || {
            for _ in 0..1024 {
                a = _mm_xor_si128(_mm_clmulepi64_si128::<0x00>(a, b), _mm_srli_epi64::<1>(a));
            }
            black_box(a);
            1024
        });
        black_box(a);
        ops as f64 / e.as_secs_f64() / 1e9
    }

    #[target_feature(enable = "sha")]
    pub unsafe fn sha(duration: Duration) -> f64 {
        let mut a = _mm_set1_epi32(black_box(0x12345678));
        let mut b = _mm_set1_epi32(0x5a827999);
        let k = _mm_set1_epi32(0x6ed9eba1);
        let (rounds, e) = timed_chunks(duration, || {
            for _ in 0..1024 {
                a = _mm_sha256rnds2_epu32(a, b, k);
                b = _mm_shuffle_epi32::<0x4e>(a);
            }
            black_box((a, b));
            2048
        });
        black_box((a, b));
        rounds as f64 / e.as_secs_f64() / 1e9
    }

    #[target_feature(enable = "adx")]
    pub unsafe fn adx(duration: Duration) -> f64 {
        let mut a = black_box(0x123456789abcdef0u64);
        let mut out = 0u64;
        let (ops, e) = timed_chunks(duration, || {
            let mut carry = 0u8;
            for _ in 0..2048 {
                carry = _addcarryx_u64(carry, a, 0x9e3779b97f4a7c15, &mut out);
                a = out.rotate_left(7) ^ carry as u64;
            }
            black_box((a, out, carry));
            2048
        });
        black_box((a, out));
        ops as f64 / e.as_secs_f64() / 1e9
    }

    #[target_feature(enable = "rdrand")]
    pub unsafe fn rdrand(duration: Duration) -> f64 {
        let mut value = 0u64;
        let (samples, e) = timed_chunks(duration, || {
            let mut ok = 0u64;
            for _ in 0..256 {
                ok += _rdrand64_step(&mut value) as u64;
            }
            black_box(value);
            ok
        });
        black_box(value);
        samples as f64 / e.as_secs_f64() / 1e6
    }

    #[target_feature(enable = "rdseed")]
    pub unsafe fn rdseed(duration: Duration) -> f64 {
        let mut value = 0u64;
        let (samples, e) = timed_chunks(duration, || {
            let mut ok = 0u64;
            for _ in 0..64 {
                ok += _rdseed64_step(&mut value) as u64;
            }
            black_box(value);
            ok
        });
        black_box(value);
        samples as f64 / e.as_secs_f64() / 1e6
    }
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn neon(duration: Duration) -> f64 {
    use std::arch::aarch64::*;
    let mut a = vdupq_n_f32(black_box(1.000001));
    let b = vdupq_n_f32(1.0000001);
    let c = vdupq_n_f32(0.000001);
    let (ops, e) = timed_chunks(duration, || {
        for _ in 0..512 {
            a = vfmaq_f32(c, a, b);
        }
        black_box(a);
        512 * 4 * 2
    });
    black_box(a);
    ops as f64 / e.as_secs_f64() / 1e9
}

pub fn run_acceleration(duration: Duration) -> (BTreeMap<String, f64>, BTreeMap<String, String>) {
    let mut out = BTreeMap::new();
    let mut units = BTreeMap::new();
    macro_rules! put {
        ($name:expr,$value:expr,$unit:expr) => {{
            out.insert($name.into(), $value);
            units.insert($name.into(), $unit.into());
        }};
    }
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    unsafe {
        if std::is_x86_feature_detected!("sse2") {
            put!("SSE2 FP", x86kernels::sse2(duration), "GFLOP/s");
        }
        if std::is_x86_feature_detected!("avx2") {
            put!("AVX2 integer", x86kernels::avx2(duration), "Gop/s");
        }
        if std::is_x86_feature_detected!("avx2") && std::is_x86_feature_detected!("fma") {
            put!("FMA", x86kernels::fma(duration), "GFLOP/s");
        }
        if std::is_x86_feature_detected!("aes") {
            put!("AES-NI", x86kernels::aes(duration), "G round/s");
        }
        if std::is_x86_feature_detected!("sse4.2") {
            put!("SSE4.2 CRC", x86kernels::crc(duration), "GB/s");
        }
        if std::is_x86_feature_detected!("popcnt") {
            put!("POPCNT", x86kernels::popcnt(duration), "Gop/s");
        }
        if std::is_x86_feature_detected!("bmi2") {
            put!("BMI2 PDEP", x86kernels::bmi2(duration), "Gop/s");
        }
        if std::is_x86_feature_detected!("bmi1") {
            put!("BMI1 bit ops", x86kernels::bmi1(duration), "Gop/s");
        }
        if std::is_x86_feature_detected!("pclmulqdq") {
            put!("PCLMULQDQ", x86kernels::pclmul(duration), "Gop/s");
        }
        if std::is_x86_feature_detected!("sha") {
            put!("SHA-NI", x86kernels::sha(duration), "G round/s");
        }
        if std::is_x86_feature_detected!("adx") {
            put!("ADX carry", x86kernels::adx(duration), "Gop/s");
        }
        if std::is_x86_feature_detected!("rdrand") {
            put!("RDRAND", x86kernels::rdrand(duration), "M sample/s");
        }
        if std::is_x86_feature_detected!("rdseed") {
            put!("RDSEED", x86kernels::rdseed(duration), "M sample/s");
        }
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        if std::arch::is_aarch64_feature_detected!("neon") {
            put!("NEON FP", neon(duration), "GFLOP/s");
        }
    }
    (out, units)
}
