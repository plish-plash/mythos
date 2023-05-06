use crate::spec::{Cc, LinkerFlavor, Lld, RelroLevel, TargetOptions, SanitizerSet, PanicStrategy, CodeModel};

pub fn opts() -> TargetOptions {
    TargetOptions {
        os: "mythos".into(),
        linker_flavor: LinkerFlavor::Gnu(Cc::No, Lld::Yes),
        linker: Some("rust-lld".into()),
        //dynamic_linking: true,
        //has_rpath: true,
        position_independent_executables: true,
        static_position_independent_executables: true,
        relro_level: RelroLevel::Full,
        //has_thread_local: true,
        //crt_static_respected: true,
        features:
        "-mmx,-sse,-sse2,-sse3,-ssse3,-sse4.1,-sse4.2,-3dnow,-3dnowa,-avx,-avx2,+soft-float"
            .into(),
        supported_sanitizers: SanitizerSet::KCFI | SanitizerSet::KERNELADDRESS,
        disable_redzone: true,
        panic_strategy: PanicStrategy::Abort,
        code_model: Some(CodeModel::Kernel),

        dll_prefix: "".into(),
        dll_suffix: ".dylib".into(),
        exe_suffix: ".bin".into(),
        staticlib_prefix: "".into(),
        staticlib_suffix: ".stlib".into(),
        ..Default::default()
    }
}
