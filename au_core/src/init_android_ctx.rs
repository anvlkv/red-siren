use jni::{
    signature::ReturnType,
    sys::{jint, jsize, JavaVM},
};
use std::{ffi::c_void, ptr::null_mut};

pub type JniGetCreatedJavaVms =
    unsafe extern "system" fn(vmBuf: *mut *mut JavaVM, bufLen: jsize, nVMs: *mut jsize) -> jint;
pub const JNI_GET_JAVA_VMS_NAME: &[u8] = b"JNI_GetCreatedJavaVMs";

pub unsafe fn initialize_android_context() {
    let lib = libloading::os::unix::Library::this();
    let get_created_java_vms: JniGetCreatedJavaVms =
        unsafe { *lib.get(JNI_GET_JAVA_VMS_NAME).unwrap() };
    let mut created_java_vms: [*mut JavaVM; 1] = [null_mut() as *mut JavaVM];
    let mut java_vms_count: i32 = 0;
    unsafe {
        get_created_java_vms(created_java_vms.as_mut_ptr(), 1, &mut java_vms_count);
    }
    let jvm_ptr = *created_java_vms.first().unwrap();
    let jvm = unsafe { jni::JavaVM::from_raw(jvm_ptr) }.unwrap();
    let mut env = jvm.get_env().unwrap();

    let activity_thread = env.find_class("android/app/ActivityThread").unwrap();
    let current_activity_thread = env
        .get_static_method_id(
            &activity_thread,
            "currentActivityThread",
            "()Landroid/app/ActivityThread;",
        )
        .unwrap();
    let at = env
        .call_static_method_unchecked(
            &activity_thread,
            current_activity_thread,
            ReturnType::Object,
            &[],
        )
        .unwrap();

    let get_application = env
        .get_method_id(
            activity_thread,
            "getApplication",
            "()Landroid/app/Application;",
        )
        .unwrap();
    let context = env
        .call_method_unchecked(at.l().unwrap(), get_application, ReturnType::Object, &[])
        .unwrap();

    ndk_context::initialize_android_context(
        jvm.get_java_vm_pointer() as *mut c_void,
        context.l().unwrap().to_owned() as *mut c_void,
    );
}
