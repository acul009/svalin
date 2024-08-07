// This file is automatically generated, so please do not edit it.
// Generated by `flutter_rust_bridge`@ 2.2.0.

#![allow(
    non_camel_case_types,
    unused,
    non_snake_case,
    clippy::needless_return,
    clippy::redundant_closure_call,
    clippy::redundant_closure,
    clippy::useless_conversion,
    clippy::unit_arg,
    clippy::unused_unit,
    clippy::double_parens,
    clippy::let_and_return,
    clippy::too_many_arguments,
    clippy::match_single_binding,
    clippy::clone_on_copy,
    clippy::let_unit_value,
    clippy::deref_addrof,
    clippy::explicit_auto_deref,
    clippy::borrow_deref_ref,
    clippy::needless_borrow
)]

// Section: imports

use flutter_rust_bridge::for_generated::byteorder::{NativeEndian, ReadBytesExt, WriteBytesExt};
use flutter_rust_bridge::for_generated::{transform_result_dco, Lifetimeable, Lockable};
use flutter_rust_bridge::{Handler, IntoIntoDart};

// Section: boilerplate

flutter_rust_bridge::frb_generated_boilerplate!(
    default_stream_sink_codec = SseCodec,
    default_rust_opaque = RustOpaqueMoi,
    default_rust_auto_opaque = RustAutoOpaqueMoi,
);
pub(crate) const FLUTTER_RUST_BRIDGE_CODEGEN_VERSION: &str = "2.2.0";
pub(crate) const FLUTTER_RUST_BRIDGE_CODEGEN_CONTENT_HASH: i32 = 961483730;

// Section: executor

flutter_rust_bridge::frb_generated_default_handler!();

// Section: wire_funcs

fn wire__crate__api__simple__greet_impl(
    ptr_: flutter_rust_bridge::for_generated::PlatformGeneralizedUint8ListPtr,
    rust_vec_len_: i32,
    data_len_: i32,
) -> flutter_rust_bridge::for_generated::WireSyncRust2DartSse {
    FLUTTER_RUST_BRIDGE_HANDLER.wrap_sync::<flutter_rust_bridge::for_generated::SseCodec, _>(
        flutter_rust_bridge::for_generated::TaskInfo {
            debug_name: "greet",
            port: None,
            mode: flutter_rust_bridge::for_generated::FfiCallMode::Sync,
        },
        move || {
            let message = unsafe {
                flutter_rust_bridge::for_generated::Dart2RustMessageSse::from_wire(
                    ptr_,
                    rust_vec_len_,
                    data_len_,
                )
            };
            let mut deserializer =
                flutter_rust_bridge::for_generated::SseDeserializer::new(message);
            let api_name = <String>::sse_decode(&mut deserializer);
            deserializer.end();
            transform_result_sse::<_, ()>((move || {
                let output_ok = Result::<_, ()>::Ok(crate::api::simple::greet(api_name))?;
                Ok(output_ok)
            })())
        },
    )
}
fn wire__crate__api__simple__init_app_impl(
    port_: flutter_rust_bridge::for_generated::MessagePort,
    ptr_: flutter_rust_bridge::for_generated::PlatformGeneralizedUint8ListPtr,
    rust_vec_len_: i32,
    data_len_: i32,
) {
    FLUTTER_RUST_BRIDGE_HANDLER.wrap_normal::<flutter_rust_bridge::for_generated::SseCodec, _, _>(
        flutter_rust_bridge::for_generated::TaskInfo {
            debug_name: "init_app",
            port: Some(port_),
            mode: flutter_rust_bridge::for_generated::FfiCallMode::Normal,
        },
        move || {
            let message = unsafe {
                flutter_rust_bridge::for_generated::Dart2RustMessageSse::from_wire(
                    ptr_,
                    rust_vec_len_,
                    data_len_,
                )
            };
            let mut deserializer =
                flutter_rust_bridge::for_generated::SseDeserializer::new(message);
            deserializer.end();
            move |context| {
                transform_result_sse::<_, ()>((move || {
                    let output_ok = Result::<_, ()>::Ok({
                        crate::api::simple::init_app();
                    })?;
                    Ok(output_ok)
                })())
            }
        },
    )
}
fn wire__svalin_sysctl__realtime__realtime_status_get_impl(
    port_: flutter_rust_bridge::for_generated::MessagePort,
    ptr_: flutter_rust_bridge::for_generated::PlatformGeneralizedUint8ListPtr,
    rust_vec_len_: i32,
    data_len_: i32,
) {
    FLUTTER_RUST_BRIDGE_HANDLER.wrap_async::<flutter_rust_bridge::for_generated::SseCodec, _, _, _>(
        flutter_rust_bridge::for_generated::TaskInfo {
            debug_name: "realtime_status_get",
            port: Some(port_),
            mode: flutter_rust_bridge::for_generated::FfiCallMode::Normal,
        },
        move || {
            let message = unsafe {
                flutter_rust_bridge::for_generated::Dart2RustMessageSse::from_wire(
                    ptr_,
                    rust_vec_len_,
                    data_len_,
                )
            };
            let mut deserializer =
                flutter_rust_bridge::for_generated::SseDeserializer::new(message);
            deserializer.end();
            move |context| async move {
                transform_result_sse::<_, ()>(
                    (move || async move {
                        let output_ok = Result::<_, ()>::Ok(
                            svalin_sysctl::realtime::RealtimeStatus::get().await,
                        )?;
                        Ok(output_ok)
                    })()
                    .await,
                )
            }
        },
    )
}

// Section: static_checks

#[allow(clippy::unnecessary_literal_unwrap)]
const _: fn() = || {
    {
        let CoreStatus = None::<svalin_sysctl::realtime::CoreStatus>.unwrap();
        let _: f32 = CoreStatus.load;
        let _: u64 = CoreStatus.frequency;
    }
    {
        let CpuStatus = None::<svalin_sysctl::realtime::CpuStatus>.unwrap();
        let _: Vec<svalin_sysctl::realtime::CoreStatus> = CpuStatus.cores;
    }
    {
        let MemoryStatus = None::<svalin_sysctl::realtime::MemoryStatus>.unwrap();
        let _: u64 = MemoryStatus.total;
        let _: u64 = MemoryStatus.available;
        let _: u64 = MemoryStatus.free;
        let _: u64 = MemoryStatus.used;
    }
    {
        let RealtimeStatus = None::<svalin_sysctl::realtime::RealtimeStatus>.unwrap();
        let _: svalin_sysctl::realtime::CpuStatus = RealtimeStatus.cpu;
        let _: svalin_sysctl::realtime::MemoryStatus = RealtimeStatus.memory;
        let _: svalin_sysctl::realtime::SwapStatus = RealtimeStatus.swap;
    }
    {
        let SwapStatus = None::<svalin_sysctl::realtime::SwapStatus>.unwrap();
        let _: u64 = SwapStatus.total;
        let _: u64 = SwapStatus.free;
        let _: u64 = SwapStatus.used;
    }
};

// Section: dart2rust

impl SseDecode for String {
    // Codec=Sse (Serialization based), see doc to use other codecs
    fn sse_decode(deserializer: &mut flutter_rust_bridge::for_generated::SseDeserializer) -> Self {
        let mut inner = <Vec<u8>>::sse_decode(deserializer);
        return String::from_utf8(inner).unwrap();
    }
}

impl SseDecode for svalin_sysctl::realtime::CoreStatus {
    // Codec=Sse (Serialization based), see doc to use other codecs
    fn sse_decode(deserializer: &mut flutter_rust_bridge::for_generated::SseDeserializer) -> Self {
        let mut var_load = <f32>::sse_decode(deserializer);
        let mut var_frequency = <u64>::sse_decode(deserializer);
        return svalin_sysctl::realtime::CoreStatus {
            load: var_load,
            frequency: var_frequency,
        };
    }
}

impl SseDecode for svalin_sysctl::realtime::CpuStatus {
    // Codec=Sse (Serialization based), see doc to use other codecs
    fn sse_decode(deserializer: &mut flutter_rust_bridge::for_generated::SseDeserializer) -> Self {
        let mut var_cores = <Vec<svalin_sysctl::realtime::CoreStatus>>::sse_decode(deserializer);
        return svalin_sysctl::realtime::CpuStatus { cores: var_cores };
    }
}

impl SseDecode for f32 {
    // Codec=Sse (Serialization based), see doc to use other codecs
    fn sse_decode(deserializer: &mut flutter_rust_bridge::for_generated::SseDeserializer) -> Self {
        deserializer.cursor.read_f32::<NativeEndian>().unwrap()
    }
}

impl SseDecode for Vec<svalin_sysctl::realtime::CoreStatus> {
    // Codec=Sse (Serialization based), see doc to use other codecs
    fn sse_decode(deserializer: &mut flutter_rust_bridge::for_generated::SseDeserializer) -> Self {
        let mut len_ = <i32>::sse_decode(deserializer);
        let mut ans_ = vec![];
        for idx_ in 0..len_ {
            ans_.push(<svalin_sysctl::realtime::CoreStatus>::sse_decode(
                deserializer,
            ));
        }
        return ans_;
    }
}

impl SseDecode for Vec<u8> {
    // Codec=Sse (Serialization based), see doc to use other codecs
    fn sse_decode(deserializer: &mut flutter_rust_bridge::for_generated::SseDeserializer) -> Self {
        let mut len_ = <i32>::sse_decode(deserializer);
        let mut ans_ = vec![];
        for idx_ in 0..len_ {
            ans_.push(<u8>::sse_decode(deserializer));
        }
        return ans_;
    }
}

impl SseDecode for svalin_sysctl::realtime::MemoryStatus {
    // Codec=Sse (Serialization based), see doc to use other codecs
    fn sse_decode(deserializer: &mut flutter_rust_bridge::for_generated::SseDeserializer) -> Self {
        let mut var_total = <u64>::sse_decode(deserializer);
        let mut var_available = <u64>::sse_decode(deserializer);
        let mut var_free = <u64>::sse_decode(deserializer);
        let mut var_used = <u64>::sse_decode(deserializer);
        return svalin_sysctl::realtime::MemoryStatus {
            total: var_total,
            available: var_available,
            free: var_free,
            used: var_used,
        };
    }
}

impl SseDecode for svalin_sysctl::realtime::RealtimeStatus {
    // Codec=Sse (Serialization based), see doc to use other codecs
    fn sse_decode(deserializer: &mut flutter_rust_bridge::for_generated::SseDeserializer) -> Self {
        let mut var_cpu = <svalin_sysctl::realtime::CpuStatus>::sse_decode(deserializer);
        let mut var_memory = <svalin_sysctl::realtime::MemoryStatus>::sse_decode(deserializer);
        let mut var_swap = <svalin_sysctl::realtime::SwapStatus>::sse_decode(deserializer);
        return svalin_sysctl::realtime::RealtimeStatus {
            cpu: var_cpu,
            memory: var_memory,
            swap: var_swap,
        };
    }
}

impl SseDecode for svalin_sysctl::realtime::SwapStatus {
    // Codec=Sse (Serialization based), see doc to use other codecs
    fn sse_decode(deserializer: &mut flutter_rust_bridge::for_generated::SseDeserializer) -> Self {
        let mut var_total = <u64>::sse_decode(deserializer);
        let mut var_free = <u64>::sse_decode(deserializer);
        let mut var_used = <u64>::sse_decode(deserializer);
        return svalin_sysctl::realtime::SwapStatus {
            total: var_total,
            free: var_free,
            used: var_used,
        };
    }
}

impl SseDecode for u64 {
    // Codec=Sse (Serialization based), see doc to use other codecs
    fn sse_decode(deserializer: &mut flutter_rust_bridge::for_generated::SseDeserializer) -> Self {
        deserializer.cursor.read_u64::<NativeEndian>().unwrap()
    }
}

impl SseDecode for u8 {
    // Codec=Sse (Serialization based), see doc to use other codecs
    fn sse_decode(deserializer: &mut flutter_rust_bridge::for_generated::SseDeserializer) -> Self {
        deserializer.cursor.read_u8().unwrap()
    }
}

impl SseDecode for () {
    // Codec=Sse (Serialization based), see doc to use other codecs
    fn sse_decode(deserializer: &mut flutter_rust_bridge::for_generated::SseDeserializer) -> Self {}
}

impl SseDecode for i32 {
    // Codec=Sse (Serialization based), see doc to use other codecs
    fn sse_decode(deserializer: &mut flutter_rust_bridge::for_generated::SseDeserializer) -> Self {
        deserializer.cursor.read_i32::<NativeEndian>().unwrap()
    }
}

impl SseDecode for bool {
    // Codec=Sse (Serialization based), see doc to use other codecs
    fn sse_decode(deserializer: &mut flutter_rust_bridge::for_generated::SseDeserializer) -> Self {
        deserializer.cursor.read_u8().unwrap() != 0
    }
}

fn pde_ffi_dispatcher_primary_impl(
    func_id: i32,
    port: flutter_rust_bridge::for_generated::MessagePort,
    ptr: flutter_rust_bridge::for_generated::PlatformGeneralizedUint8ListPtr,
    rust_vec_len: i32,
    data_len: i32,
) {
    // Codec=Pde (Serialization + dispatch), see doc to use other codecs
    match func_id {
        2 => wire__crate__api__simple__init_app_impl(port, ptr, rust_vec_len, data_len),
        3 => wire__svalin_sysctl__realtime__realtime_status_get_impl(
            port,
            ptr,
            rust_vec_len,
            data_len,
        ),
        _ => unreachable!(),
    }
}

fn pde_ffi_dispatcher_sync_impl(
    func_id: i32,
    ptr: flutter_rust_bridge::for_generated::PlatformGeneralizedUint8ListPtr,
    rust_vec_len: i32,
    data_len: i32,
) -> flutter_rust_bridge::for_generated::WireSyncRust2DartSse {
    // Codec=Pde (Serialization + dispatch), see doc to use other codecs
    match func_id {
        1 => wire__crate__api__simple__greet_impl(ptr, rust_vec_len, data_len),
        _ => unreachable!(),
    }
}

// Section: rust2dart

// Codec=Dco (DartCObject based), see doc to use other codecs
impl flutter_rust_bridge::IntoDart for FrbWrapper<svalin_sysctl::realtime::CoreStatus> {
    fn into_dart(self) -> flutter_rust_bridge::for_generated::DartAbi {
        [
            self.0.load.into_into_dart().into_dart(),
            self.0.frequency.into_into_dart().into_dart(),
        ]
        .into_dart()
    }
}
impl flutter_rust_bridge::for_generated::IntoDartExceptPrimitive
    for FrbWrapper<svalin_sysctl::realtime::CoreStatus>
{
}
impl flutter_rust_bridge::IntoIntoDart<FrbWrapper<svalin_sysctl::realtime::CoreStatus>>
    for svalin_sysctl::realtime::CoreStatus
{
    fn into_into_dart(self) -> FrbWrapper<svalin_sysctl::realtime::CoreStatus> {
        self.into()
    }
}
// Codec=Dco (DartCObject based), see doc to use other codecs
impl flutter_rust_bridge::IntoDart for FrbWrapper<svalin_sysctl::realtime::CpuStatus> {
    fn into_dart(self) -> flutter_rust_bridge::for_generated::DartAbi {
        [self.0.cores.into_into_dart().into_dart()].into_dart()
    }
}
impl flutter_rust_bridge::for_generated::IntoDartExceptPrimitive
    for FrbWrapper<svalin_sysctl::realtime::CpuStatus>
{
}
impl flutter_rust_bridge::IntoIntoDart<FrbWrapper<svalin_sysctl::realtime::CpuStatus>>
    for svalin_sysctl::realtime::CpuStatus
{
    fn into_into_dart(self) -> FrbWrapper<svalin_sysctl::realtime::CpuStatus> {
        self.into()
    }
}
// Codec=Dco (DartCObject based), see doc to use other codecs
impl flutter_rust_bridge::IntoDart for FrbWrapper<svalin_sysctl::realtime::MemoryStatus> {
    fn into_dart(self) -> flutter_rust_bridge::for_generated::DartAbi {
        [
            self.0.total.into_into_dart().into_dart(),
            self.0.available.into_into_dart().into_dart(),
            self.0.free.into_into_dart().into_dart(),
            self.0.used.into_into_dart().into_dart(),
        ]
        .into_dart()
    }
}
impl flutter_rust_bridge::for_generated::IntoDartExceptPrimitive
    for FrbWrapper<svalin_sysctl::realtime::MemoryStatus>
{
}
impl flutter_rust_bridge::IntoIntoDart<FrbWrapper<svalin_sysctl::realtime::MemoryStatus>>
    for svalin_sysctl::realtime::MemoryStatus
{
    fn into_into_dart(self) -> FrbWrapper<svalin_sysctl::realtime::MemoryStatus> {
        self.into()
    }
}
// Codec=Dco (DartCObject based), see doc to use other codecs
impl flutter_rust_bridge::IntoDart for FrbWrapper<svalin_sysctl::realtime::RealtimeStatus> {
    fn into_dart(self) -> flutter_rust_bridge::for_generated::DartAbi {
        [
            self.0.cpu.into_into_dart().into_dart(),
            self.0.memory.into_into_dart().into_dart(),
            self.0.swap.into_into_dart().into_dart(),
        ]
        .into_dart()
    }
}
impl flutter_rust_bridge::for_generated::IntoDartExceptPrimitive
    for FrbWrapper<svalin_sysctl::realtime::RealtimeStatus>
{
}
impl flutter_rust_bridge::IntoIntoDart<FrbWrapper<svalin_sysctl::realtime::RealtimeStatus>>
    for svalin_sysctl::realtime::RealtimeStatus
{
    fn into_into_dart(self) -> FrbWrapper<svalin_sysctl::realtime::RealtimeStatus> {
        self.into()
    }
}
// Codec=Dco (DartCObject based), see doc to use other codecs
impl flutter_rust_bridge::IntoDart for FrbWrapper<svalin_sysctl::realtime::SwapStatus> {
    fn into_dart(self) -> flutter_rust_bridge::for_generated::DartAbi {
        [
            self.0.total.into_into_dart().into_dart(),
            self.0.free.into_into_dart().into_dart(),
            self.0.used.into_into_dart().into_dart(),
        ]
        .into_dart()
    }
}
impl flutter_rust_bridge::for_generated::IntoDartExceptPrimitive
    for FrbWrapper<svalin_sysctl::realtime::SwapStatus>
{
}
impl flutter_rust_bridge::IntoIntoDart<FrbWrapper<svalin_sysctl::realtime::SwapStatus>>
    for svalin_sysctl::realtime::SwapStatus
{
    fn into_into_dart(self) -> FrbWrapper<svalin_sysctl::realtime::SwapStatus> {
        self.into()
    }
}

impl SseEncode for String {
    // Codec=Sse (Serialization based), see doc to use other codecs
    fn sse_encode(self, serializer: &mut flutter_rust_bridge::for_generated::SseSerializer) {
        <Vec<u8>>::sse_encode(self.into_bytes(), serializer);
    }
}

impl SseEncode for svalin_sysctl::realtime::CoreStatus {
    // Codec=Sse (Serialization based), see doc to use other codecs
    fn sse_encode(self, serializer: &mut flutter_rust_bridge::for_generated::SseSerializer) {
        <f32>::sse_encode(self.load, serializer);
        <u64>::sse_encode(self.frequency, serializer);
    }
}

impl SseEncode for svalin_sysctl::realtime::CpuStatus {
    // Codec=Sse (Serialization based), see doc to use other codecs
    fn sse_encode(self, serializer: &mut flutter_rust_bridge::for_generated::SseSerializer) {
        <Vec<svalin_sysctl::realtime::CoreStatus>>::sse_encode(self.cores, serializer);
    }
}

impl SseEncode for f32 {
    // Codec=Sse (Serialization based), see doc to use other codecs
    fn sse_encode(self, serializer: &mut flutter_rust_bridge::for_generated::SseSerializer) {
        serializer.cursor.write_f32::<NativeEndian>(self).unwrap();
    }
}

impl SseEncode for Vec<svalin_sysctl::realtime::CoreStatus> {
    // Codec=Sse (Serialization based), see doc to use other codecs
    fn sse_encode(self, serializer: &mut flutter_rust_bridge::for_generated::SseSerializer) {
        <i32>::sse_encode(self.len() as _, serializer);
        for item in self {
            <svalin_sysctl::realtime::CoreStatus>::sse_encode(item, serializer);
        }
    }
}

impl SseEncode for Vec<u8> {
    // Codec=Sse (Serialization based), see doc to use other codecs
    fn sse_encode(self, serializer: &mut flutter_rust_bridge::for_generated::SseSerializer) {
        <i32>::sse_encode(self.len() as _, serializer);
        for item in self {
            <u8>::sse_encode(item, serializer);
        }
    }
}

impl SseEncode for svalin_sysctl::realtime::MemoryStatus {
    // Codec=Sse (Serialization based), see doc to use other codecs
    fn sse_encode(self, serializer: &mut flutter_rust_bridge::for_generated::SseSerializer) {
        <u64>::sse_encode(self.total, serializer);
        <u64>::sse_encode(self.available, serializer);
        <u64>::sse_encode(self.free, serializer);
        <u64>::sse_encode(self.used, serializer);
    }
}

impl SseEncode for svalin_sysctl::realtime::RealtimeStatus {
    // Codec=Sse (Serialization based), see doc to use other codecs
    fn sse_encode(self, serializer: &mut flutter_rust_bridge::for_generated::SseSerializer) {
        <svalin_sysctl::realtime::CpuStatus>::sse_encode(self.cpu, serializer);
        <svalin_sysctl::realtime::MemoryStatus>::sse_encode(self.memory, serializer);
        <svalin_sysctl::realtime::SwapStatus>::sse_encode(self.swap, serializer);
    }
}

impl SseEncode for svalin_sysctl::realtime::SwapStatus {
    // Codec=Sse (Serialization based), see doc to use other codecs
    fn sse_encode(self, serializer: &mut flutter_rust_bridge::for_generated::SseSerializer) {
        <u64>::sse_encode(self.total, serializer);
        <u64>::sse_encode(self.free, serializer);
        <u64>::sse_encode(self.used, serializer);
    }
}

impl SseEncode for u64 {
    // Codec=Sse (Serialization based), see doc to use other codecs
    fn sse_encode(self, serializer: &mut flutter_rust_bridge::for_generated::SseSerializer) {
        serializer.cursor.write_u64::<NativeEndian>(self).unwrap();
    }
}

impl SseEncode for u8 {
    // Codec=Sse (Serialization based), see doc to use other codecs
    fn sse_encode(self, serializer: &mut flutter_rust_bridge::for_generated::SseSerializer) {
        serializer.cursor.write_u8(self).unwrap();
    }
}

impl SseEncode for () {
    // Codec=Sse (Serialization based), see doc to use other codecs
    fn sse_encode(self, serializer: &mut flutter_rust_bridge::for_generated::SseSerializer) {}
}

impl SseEncode for i32 {
    // Codec=Sse (Serialization based), see doc to use other codecs
    fn sse_encode(self, serializer: &mut flutter_rust_bridge::for_generated::SseSerializer) {
        serializer.cursor.write_i32::<NativeEndian>(self).unwrap();
    }
}

impl SseEncode for bool {
    // Codec=Sse (Serialization based), see doc to use other codecs
    fn sse_encode(self, serializer: &mut flutter_rust_bridge::for_generated::SseSerializer) {
        serializer.cursor.write_u8(self as _).unwrap();
    }
}

#[cfg(not(target_family = "wasm"))]
mod io {
    // This file is automatically generated, so please do not edit it.
    // Generated by `flutter_rust_bridge`@ 2.2.0.

    // Section: imports

    use super::*;
    use flutter_rust_bridge::for_generated::byteorder::{
        NativeEndian, ReadBytesExt, WriteBytesExt,
    };
    use flutter_rust_bridge::for_generated::{transform_result_dco, Lifetimeable, Lockable};
    use flutter_rust_bridge::{Handler, IntoIntoDart};

    // Section: boilerplate

    flutter_rust_bridge::frb_generated_boilerplate_io!();
}
#[cfg(not(target_family = "wasm"))]
pub use io::*;

/// cbindgen:ignore
#[cfg(target_family = "wasm")]
mod web {
    // This file is automatically generated, so please do not edit it.
    // Generated by `flutter_rust_bridge`@ 2.2.0.

    // Section: imports

    use super::*;
    use flutter_rust_bridge::for_generated::byteorder::{
        NativeEndian, ReadBytesExt, WriteBytesExt,
    };
    use flutter_rust_bridge::for_generated::wasm_bindgen;
    use flutter_rust_bridge::for_generated::wasm_bindgen::prelude::*;
    use flutter_rust_bridge::for_generated::{transform_result_dco, Lifetimeable, Lockable};
    use flutter_rust_bridge::{Handler, IntoIntoDart};

    // Section: boilerplate

    flutter_rust_bridge::frb_generated_boilerplate_web!();
}
#[cfg(target_family = "wasm")]
pub use web::*;