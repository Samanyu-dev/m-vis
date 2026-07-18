#[cfg(target_os = "windows")]
mod windows {
    use windows::Win32::Foundation::{DBG_CONTINUE, DBG_EXCEPTION_NOT_HANDLED, HANDLE};
    use windows::Win32::System::Diagnostics::Debug::{
        ContinueDebugEvent, DEBUG_EVENT, DebugActiveProcess, DebugActiveProcessStop,
        EXCEPTION_DEBUG_EVENT, CREATE_THREAD_DEBUG_EVENT, EXIT_PROCESS_DEBUG_EVENT,
        WaitForDebugEvent, GetThreadContext, SetThreadContext, CONTEXT, EXCEPTION_RECORD,
    };
    use windows::Win32::System::LibraryLoader::{GetProcAddress, GetModuleHandleA};
    use windows::Win32::System::Threading::{OpenThread, OpenProcess, THREAD_ALL_ACCESS, PROCESS_ALL_ACCESS, SuspendThread, ResumeThread, CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32};
    use windows::core::s;
    use crate::types::{AllocationTrace, AllocationEvent};
    use crate::core::stack_trace::{StackTrace, capture_from_context};
    use std::collections::HashMap;

    pub fn trace_allocations(pid: u32, duration_secs: u64, regions: &[crate::types::Region]) -> Result<Vec<AllocationTrace>, String> {
        let mut traces = Vec::new();
        unsafe {
            let h_mod = GetModuleHandleA(s!("ntdll.dll")).map_err(|e| e.to_string())?;
            let rtl_allocate_heap = GetProcAddress(h_mod, s!("RtlAllocateHeap")).ok_or("Cannot find RtlAllocateHeap")?;
            let bp_addr = rtl_allocate_heap as u64;

            DebugActiveProcess(pid).map_err(|e| format!("failed to attach: {}", e))?;
            println!("Attached to pid {} for {}s. Intercepting RtlAllocateHeap at 0x{:x}", pid, duration_secs, bp_addr);

            // Set HW breakpoint on all existing threads
            let proc_handle = OpenProcess(PROCESS_ALL_ACCESS, false, pid).unwrap();
            let h_snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0).unwrap();
            let mut te32 = THREADENTRY32::default();
            te32.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
            
            if Thread32First(h_snapshot, &mut te32).as_bool() {
                loop {
                    if te32.th32OwnerProcessID == pid {
                        if let Ok(th) = OpenThread(THREAD_ALL_ACCESS, false, te32.th32ThreadID) {
                            SuspendThread(th);
                            let mut ctx = CONTEXT::default();
                            ctx.ContextFlags = windows::Win32::System::Diagnostics::Debug::CONTEXT_ALL;
                            if GetThreadContext(th, &mut ctx).is_ok() {
                                ctx.Dr0 = bp_addr;
                                ctx.Dr7 |= 1; // enable local DR0
                                let _ = SetThreadContext(th, &ctx);
                            }
                            ResumeThread(th);
                            windows::Win32::Foundation::CloseHandle(th);
                        }
                    }
                    if !Thread32Next(h_snapshot, &mut te32).as_bool() {
                        break;
                    }
                }
            }
            windows::Win32::Foundation::CloseHandle(h_snapshot);

            let mut event = DEBUG_EVENT::default();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(duration_secs);

            loop {
                if std::time::Instant::now() > deadline {
                    break;
                }

                if !WaitForDebugEvent(&mut event, 100).as_bool() {
                    continue;
                }

                let mut continue_status = DBG_CONTINUE;

                match event.dwDebugEventCode {
                    EXIT_PROCESS_DEBUG_EVENT => {
                        break;
                    }
                    CREATE_THREAD_DEBUG_EVENT => {
                        // set breakpoint on new thread
                        let th = event.u.CreateThread.hThread;
                        let mut ctx = CONTEXT::default();
                        ctx.ContextFlags = windows::Win32::System::Diagnostics::Debug::CONTEXT_ALL;
                        if GetThreadContext(th, &mut ctx).is_ok() {
                            ctx.Dr0 = bp_addr;
                            ctx.Dr7 |= 1;
                            let _ = SetThreadContext(th, &ctx);
                        }
                    }
                    EXCEPTION_DEBUG_EVENT => {
                        let exc = &event.u.Exception.ExceptionRecord;
                        if exc.ExceptionCode == windows::Win32::Foundation::EXCEPTION_SINGLE_STEP {
                            if exc.ExceptionAddress as u64 == bp_addr {
                                // Hit RtlAllocateHeap!
                                if let Ok(th) = OpenThread(THREAD_ALL_ACCESS, false, event.dwThreadId) {
                                    let mut ctx = CONTEXT::default();
                                    ctx.ContextFlags = windows::Win32::System::Diagnostics::Debug::CONTEXT_ALL;
                                    if GetThreadContext(th, &mut ctx).is_ok() {
                                        let size = ctx.R8 as usize; // arg 3
                                        // Capture stack trace
                                        if let Ok(frames) = capture_from_context(proc_handle, th, &mut ctx, regions) {
                                            traces.push(AllocationTrace {
                                                event: AllocationEvent {
                                                    address: 0, // returned from RtlAllocateHeap later, but we only have size here
                                                    size,
                                                },
                                                frames,
                                            });
                                        }
                                        
                                        // Set RF flag so we can execute the instruction we broke on without triggering again
                                        ctx.EFlags |= 1 << 16;
                                        let _ = SetThreadContext(th, &ctx);
                                    }
                                    windows::Win32::Foundation::CloseHandle(th);
                                }
                                continue_status = DBG_CONTINUE;
                            } else {
                                continue_status = DBG_EXCEPTION_NOT_HANDLED;
                            }
                        } else {
                            continue_status = DBG_EXCEPTION_NOT_HANDLED;
                        }
                    }
                    _ => {
                        continue_status = DBG_EXCEPTION_NOT_HANDLED;
                    }
                }

                let _ = ContinueDebugEvent(event.dwProcessId, event.dwThreadId, continue_status);
            }

            windows::Win32::Foundation::CloseHandle(proc_handle);
            DebugActiveProcessStop(pid).ok();
        }
        Ok(traces)
    }
}
