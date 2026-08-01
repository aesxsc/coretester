use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CpuTopology {
    pub package: Option<i32>,
    pub core: Option<i32>,
    pub siblings: Vec<usize>,
}

#[cfg(target_os = "windows")]
mod platform {
    use super::CpuTopology;
    use std::ffi::c_void;

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct GroupAffinity {
        mask: usize,
        group: u16,
        reserved: [u16; 3],
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentThread() -> *mut c_void;
        fn GetActiveProcessorGroupCount() -> u16;
        fn GetActiveProcessorCount(group_number: u16) -> u32;
        fn SetThreadGroupAffinity(
            thread: *mut c_void,
            affinity: *const GroupAffinity,
            previous: *mut GroupAffinity,
        ) -> i32;
        fn GetLogicalProcessorInformationEx(
            relationship_type: i32,
            buffer: *mut u8,
            returned_length: *mut u32,
        ) -> i32;
    }

    pub struct Guard {
        previous: GroupAffinity,
        active: bool,
    }
    impl Guard {
        pub fn pin(cpu: usize) -> Result<Self, String> {
            let groups = unsafe { GetActiveProcessorGroupCount() };
            let mut base = 0usize;
            for group in 0..groups {
                let count = unsafe { GetActiveProcessorCount(group) } as usize;
                if cpu < base + count {
                    let bit = cpu - base;
                    if bit >= usize::BITS as usize {
                        return Err("processor group exceeds affinity mask".into());
                    }
                    let wanted = GroupAffinity {
                        mask: 1usize << bit,
                        group,
                        reserved: [0; 3],
                    };
                    let mut previous = GroupAffinity::default();
                    let ok = unsafe {
                        SetThreadGroupAffinity(GetCurrentThread(), &wanted, &mut previous)
                    } != 0;
                    if !ok {
                        return Err(format!("SetThreadGroupAffinity failed for CPU {cpu}"));
                    }
                    return Ok(Self {
                        previous,
                        active: true,
                    });
                }
                base += count;
            }
            Err(format!("CPU {cpu} is not active"))
        }
        pub fn is_active(&self) -> bool {
            self.active
        }
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            if self.active {
                unsafe {
                    SetThreadGroupAffinity(
                        GetCurrentThread(),
                        &self.previous,
                        std::ptr::null_mut(),
                    );
                }
            }
        }
    }

    pub fn count() -> usize {
        let groups = unsafe { GetActiveProcessorGroupCount() };
        (0..groups)
            .map(|g| unsafe { GetActiveProcessorCount(g) as usize })
            .sum::<usize>()
            .max(1)
    }

    pub fn topology(total: usize) -> Vec<CpuTopology> {
        const RELATION_PROCESSOR_CORE: i32 = 0;
        let mut length = 0u32;
        unsafe {
            GetLogicalProcessorInformationEx(
                RELATION_PROCESSOR_CORE,
                std::ptr::null_mut(),
                &mut length,
            );
        }
        if length < 32 {
            return fallback_topology(total);
        }
        let mut data = vec![0u8; length as usize];
        if unsafe {
            GetLogicalProcessorInformationEx(
                RELATION_PROCESSOR_CORE,
                data.as_mut_ptr(),
                &mut length,
            )
        } == 0
        {
            return fallback_topology(total);
        }
        let group_count = unsafe { GetActiveProcessorGroupCount() } as usize;
        let mut group_bases = vec![0usize; group_count];
        for group in 1..group_count {
            group_bases[group] = group_bases[group - 1]
                + unsafe { GetActiveProcessorCount((group - 1) as u16) } as usize;
        }
        let mut out = fallback_topology(total);
        let mut offset = 0usize;
        let mut core_id = 0i32;
        while offset + 32 <= length as usize {
            let relationship =
                unsafe { std::ptr::read_unaligned(data.as_ptr().add(offset) as *const i32) };
            let size = unsafe {
                std::ptr::read_unaligned(data.as_ptr().add(offset + 4) as *const u32) as usize
            };
            if size < 32 || offset + size > length as usize {
                break;
            }
            if relationship == RELATION_PROCESSOR_CORE {
                let masks = unsafe {
                    std::ptr::read_unaligned(data.as_ptr().add(offset + 30) as *const u16)
                } as usize;
                let mut siblings = Vec::new();
                for index in 0..masks {
                    let affinity_offset =
                        offset + 32 + index * std::mem::size_of::<GroupAffinity>();
                    if affinity_offset + std::mem::size_of::<GroupAffinity>() > offset + size {
                        break;
                    }
                    let affinity = unsafe {
                        std::ptr::read_unaligned(
                            data.as_ptr().add(affinity_offset) as *const GroupAffinity
                        )
                    };
                    let base = group_bases
                        .get(affinity.group as usize)
                        .copied()
                        .unwrap_or(0);
                    for bit in 0..usize::BITS as usize {
                        if affinity.mask & (1usize << bit) != 0 && base + bit < total {
                            siblings.push(base + bit);
                        }
                    }
                }
                siblings.sort_unstable();
                for &cpu in &siblings {
                    out[cpu] = CpuTopology {
                        package: Some(0),
                        core: Some(core_id),
                        siblings: siblings.clone(),
                    };
                }
                core_id += 1;
            }
            offset += size;
        }
        out
    }

    fn fallback_topology(total: usize) -> Vec<CpuTopology> {
        (0..total)
            .map(|cpu| CpuTopology {
                package: None,
                core: Some(cpu as i32),
                siblings: vec![cpu],
            })
            .collect()
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use super::CpuTopology;
    use std::collections::BTreeMap;
    use std::fs;
    use std::mem::MaybeUninit;

    pub struct Guard {
        previous: libc::cpu_set_t,
        active: bool,
    }
    impl Guard {
        pub fn pin(cpu: usize) -> Result<Self, String> {
            unsafe {
                let mut previous = MaybeUninit::<libc::cpu_set_t>::zeroed().assume_init();
                if libc::sched_getaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &mut previous)
                    != 0
                {
                    return Err(std::io::Error::last_os_error().to_string());
                }
                let mut wanted = MaybeUninit::<libc::cpu_set_t>::zeroed().assume_init();
                libc::CPU_ZERO(&mut wanted);
                libc::CPU_SET(cpu, &mut wanted);
                if libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &wanted) != 0
                {
                    return Err(std::io::Error::last_os_error().to_string());
                }
                Ok(Self {
                    previous,
                    active: true,
                })
            }
        }
        pub fn is_active(&self) -> bool {
            self.active
        }
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            if self.active {
                unsafe {
                    libc::sched_setaffinity(
                        0,
                        std::mem::size_of::<libc::cpu_set_t>(),
                        &self.previous,
                    );
                }
            }
        }
    }

    pub fn count() -> usize {
        unsafe {
            let n = libc::sysconf(libc::_SC_NPROCESSORS_ONLN);
            if n > 0 {
                n as usize
            } else {
                std::thread::available_parallelism()
                    .map(|v| v.get())
                    .unwrap_or(1)
            }
        }
    }

    fn read_id(cpu: usize, name: &str) -> Option<i32> {
        fs::read_to_string(format!("/sys/devices/system/cpu/cpu{cpu}/topology/{name}"))
            .ok()?
            .trim()
            .parse()
            .ok()
    }

    pub fn topology(total: usize) -> Vec<CpuTopology> {
        let ids: Vec<_> = (0..total)
            .map(|cpu| (read_id(cpu, "physical_package_id"), read_id(cpu, "core_id")))
            .collect();
        let mut groups: BTreeMap<(Option<i32>, Option<i32>), Vec<usize>> = BTreeMap::new();
        for (cpu, id) in ids.iter().enumerate() {
            groups.entry(*id).or_default().push(cpu);
        }
        ids.into_iter()
            .map(|(package, core)| CpuTopology {
                package,
                core,
                siblings: groups.get(&(package, core)).cloned().unwrap_or_default(),
            })
            .collect()
    }
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
mod platform {
    use super::CpuTopology;
    pub struct Guard {
        active: bool,
    }
    impl Guard {
        pub fn pin(_: usize) -> Result<Self, String> {
            Ok(Self { active: false })
        }
        pub fn is_active(&self) -> bool {
            self.active
        }
    }
    pub fn count() -> usize {
        std::thread::available_parallelism()
            .map(|v| v.get())
            .unwrap_or(1)
    }
    pub fn topology(total: usize) -> Vec<CpuTopology> {
        vec![CpuTopology::default(); total]
    }
}

pub struct AffinityGuard(platform::Guard);
impl AffinityGuard {
    pub fn pin(cpu: usize) -> Result<Self, String> {
        platform::Guard::pin(cpu).map(Self)
    }
    pub fn is_active(&self) -> bool {
        self.0.is_active()
    }
}

pub fn logical_cpu_count() -> usize {
    platform::count()
}
pub fn topology(total: usize) -> Vec<CpuTopology> {
    platform::topology(total)
}
