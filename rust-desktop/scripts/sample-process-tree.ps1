param([int]$RootProcessId, [string]$OutputFile, [string]$StopFile)
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Runtime.InteropServices;
public static class PetProcessSampler {
  [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)] struct Entry {
    public uint size,usage,pid; public IntPtr heap; public uint module,threads,parent; public int priority; public uint flags;
    [MarshalAs(UnmanagedType.ByValTStr,SizeConst=260)] public string name;
  }
  [StructLayout(LayoutKind.Sequential)] struct Memory {
    public uint cb,faults; public UIntPtr peakWs,ws,peakPaged,paged,peakNonPaged,nonPaged,commit,peakCommit,privateUsage,privateWs,sharedCommit;
  }
  [StructLayout(LayoutKind.Sequential)] struct Io {public ulong readOps,writeOps,otherOps,readBytes,writeBytes,otherBytes;}
  [DllImport("kernel32.dll")] static extern IntPtr CreateToolhelp32Snapshot(uint flags,uint pid);
  [DllImport("kernel32.dll",CharSet=CharSet.Unicode)] static extern bool Process32FirstW(IntPtr h,ref Entry e);
  [DllImport("kernel32.dll",CharSet=CharSet.Unicode)] static extern bool Process32NextW(IntPtr h,ref Entry e);
  [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);
  [DllImport("psapi.dll")] static extern bool GetProcessMemoryInfo(IntPtr h,ref Memory m,uint cb);
  [DllImport("kernel32.dll")] static extern bool GetProcessIoCounters(IntPtr h,out Io io);
  [DllImport("kernel32.dll")] static extern bool GetSystemTimes(out long idle,out long kernel,out long user);
  public static object Sample(int root) {
    var parents=new Dictionary<int,int>();
    IntPtr snap=CreateToolhelp32Snapshot(2,0);
    try {
      var e=new Entry();e.size=(uint)Marshal.SizeOf(e);
      if(Process32FirstW(snap,ref e))do{parents[(int)e.pid]=(int)e.parent;}while(Process32NextW(snap,ref e));
    } finally {CloseHandle(snap);}
    var ids=new HashSet<int>();ids.Add(root);
    bool change=true;while(change){change=false;foreach(var p in parents)if(ids.Contains(p.Value)&&ids.Add(p.Key))change=true;}
    var rows=new List<object>();
    foreach(int id in ids)try {
      using(var p=Process.GetProcessById(id)) {
        var m=new Memory();m.cb=(uint)Marshal.SizeOf(m);bool hasMem=GetProcessMemoryInfo(p.Handle,ref m,m.cb);
        Io io;GetProcessIoCounters(p.Handle,out io);
        rows.Add(new {pid=id,parent=parents.ContainsKey(id)?parents[id]:0,name=p.ProcessName,cpuSeconds=p.TotalProcessorTime.TotalSeconds,
          ws=p.WorkingSet64,commit=p.PrivateMemorySize64,privateWs=hasMem?(long?)m.privateWs.ToUInt64():null,
          threads=p.Threads.Count,handles=p.HandleCount,readBytes=io.readBytes,writeBytes=io.writeBytes,otherBytes=io.otherBytes});
      }
    } catch(System.Exception) {}
    long idle,kernel,user;GetSystemTimes(out idle,out kernel,out user);
    return new {utc=DateTime.UtcNow.ToString("o"),systemIdle=idle,systemTotal=kernel+user,processes=rows};
  }
}
'@
$writer = [IO.StreamWriter]::new($OutputFile,$false,[Text.UTF8Encoding]::new($false))
try {
  while (-not (Test-Path -LiteralPath $StopFile)) {
    $watch=[Diagnostics.Stopwatch]::StartNew()
    $writer.WriteLine(([PetProcessSampler]::Sample($RootProcessId) | ConvertTo-Json -Depth 6 -Compress))
    $writer.Flush()
    $remaining=1000-[int]$watch.ElapsedMilliseconds
    if ($remaining -gt 0) { Start-Sleep -Milliseconds $remaining }
  }
} finally { $writer.Dispose() }
