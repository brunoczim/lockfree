# CPU info
```
Architecture:                    x86_64
CPU op-mode(s):                  32-bit, 64-bit
Address sizes:                   39 bits physical, 48 bits virtual
Byte Order:                      Little Endian
CPU(s):                          16
On-line CPU(s) list:             0-15
Vendor ID:                       GenuineIntel
Model name:                      Intel(R) Core(TM) i7-10700K CPU @ 3.80GHz
CPU family:                      6
Model:                           165
Thread(s) per core:              2
Core(s) per socket:              8
Socket(s):                       1
Stepping:                        5
CPU(s) scaling MHz:              57%
CPU max MHz:                     5100.0000
CPU min MHz:                     800.0000
BogoMIPS:                        7602.45
Flags:                           fpu vme de pse tsc msr pae mce cx8 apic sep mtrr pge mca cmov pat pse36 clflush dts acpi mmx fxsr sse sse2 ss ht tm pbe syscall nx pdpe1gb rdtscp lm constant_tsc art arch_perfmon pebs bts rep_good nopl xtopology nonstop_tsc cpuid aperfmperf pni pclmulqdq dtes64 monitor ds_cpl vmx smx est tm2 ssse3 sdbg fma cx16 xtpr pdcm pcid sse4_1 sse4_2 x2apic movbe popcnt tsc_deadline_timer aes xsave avx f16c rdrand lahf_lm abm 3dnowprefetch cpuid_fault invpcid_single ssbd ibrs ibpb stibp ibrs_enhanced tpr_shadow vnmi flexpriority ept vpid ept_ad fsgsbase tsc_adjust bmi1 avx2 smep bmi2 erms invpcid mpx rdseed adx smap clflushopt intel_pt xsaveopt xsavec xgetbv1 xsaves dtherm ida arat pln pts pku ospke md_clear flush_l1d arch_capabilities
Virtualization:                  VT-x
L1d cache:                       256 KiB (8 instances)
L1i cache:                       256 KiB (8 instances)
L2 cache:                        2 MiB (8 instances)
L3 cache:                        16 MiB (1 instance)
NUMA node(s):                    1
NUMA node0 CPU(s):               0-15
Vulnerability Itlb multihit:     KVM: Mitigation: VMX disabled
Vulnerability L1tf:              Not affected
Vulnerability Mds:               Not affected
Vulnerability Meltdown:          Not affected
Vulnerability Mmio stale data:   Vulnerable: Clear CPU buffers attempted, no microcode; SMT vulnerable
Vulnerability Retbleed:          Mitigation; Enhanced IBRS
Vulnerability Spec store bypass: Mitigation; Speculative Store Bypass disabled via prctl
Vulnerability Spectre v1:        Mitigation; usercopy/swapgs barriers and __user pointer sanitization
Vulnerability Spectre v2:        Mitigation; Enhanced IBRS, IBPB conditional, RSB filling, PBRSB-eIBRS SW sequence
Vulnerability Srbds:             Vulnerable: No microcode
Vulnerability Tsx async abort:   Not affected
```
Running on Linux 6.1.4-arch1-1 #1 SMP PREEMPT_DYNAMIC Sat, 07 Jan 2023 15:10:07 +0000 x86_64 GNU/Linux

# Benchmarks
Benchmark code under [benchmark](benchmark) directory.
More rounds per seconds is better.

As you can see, there is a lot to improve!


## THREAD-LOCAL STORAGE
```
Result for 1 threads:
Target 0 (std/global):
mean of 851216201.175 r/s (1064085504 rounds in 1.250 seconds)
Target 1 (blocking):
mean of 433788599.293 r/s (542268416 rounds in 1.250 seconds)
Target 2 (blocking with cached access):
mean of 432815147.726 r/s (541053952 rounds in 1.250 seconds)
Target 3 (lockfree):
mean of 281114121.514 r/s (351413248 rounds in 1.250 seconds)
Target 4 (lockfree with cached id):
mean of 303622538.540 r/s (379551744 rounds in 1.250 seconds)

Result for 4 threads:
Target 0 (std/global):
mean of 3363902641.766 r/s (4205216768 rounds in 1.250 seconds)
Target 1 (blocking):
mean of 277761936.865 r/s (347229184 rounds in 1.250 seconds)
Target 2 (blocking with cached access):
mean of 185943879.551 r/s (232444928 rounds in 1.250 seconds)
Target 3 (lockfree):
mean of 1108497663.496 r/s (1385724928 rounds in 1.250 seconds)
Target 4 (lockfree with cached id):
mean of 1171230609.323 r/s (1464145920 rounds in 1.250 seconds)

Result for 16 threads:
Target 0 (std/global):
mean of 8345282740.296 r/s (10433639424 rounds in 1.250 seconds)
Target 1 (blocking):
mean of 676102912.883 r/s (845292544 rounds in 1.250 seconds)
Target 2 (blocking with cached access):
mean of 681291406.219 r/s (851744768 rounds in 1.250 seconds)
Target 3 (lockfree):
mean of 2359727923.404 r/s (2950123520 rounds in 1.250 seconds)
Target 4 (lockfree with cached id):
mean of 2203697438.159 r/s (2755062784 rounds in 1.250 seconds)

Result for 32 threads:
Target 0 (std/global):
mean of 8311773286.023 r/s (10394966016 rounds in 1.251 seconds)
Target 1 (blocking):
mean of 1465673720.646 r/s (1832720384 rounds in 1.250 seconds)
Target 2 (blocking with cached access):
mean of 1624175423.440 r/s (2030846976 rounds in 1.250 seconds)
Target 3 (lockfree):
mean of 2364188552.573 r/s (2956139520 rounds in 1.250 seconds)
Target 4 (lockfree with cached id):
mean of 2205742548.857 r/s (2758129664 rounds in 1.250 seconds)

Result for 128 threads:
Target 0 (std/global):
mean of 8333737198.803 r/s (10431406080 rounds in 1.252 seconds)
Target 1 (blocking):
mean of 2843529021.661 r/s (3559099392 rounds in 1.252 seconds)
Target 2 (blocking with cached access):
mean of 2907274859.564 r/s (3638416384 rounds in 1.251 seconds)
Target 3 (lockfree):
mean of 2342497340.538 r/s (2931552256 rounds in 1.251 seconds)
Target 4 (lockfree with cached id):
mean of 2203641854.647 r/s (2757732352 rounds in 1.251 seconds)

```

## QUEUE
```
Result for 1 threads:
Target 0 (mutex vector):
mean of 89099127.009 r/s (111380480 rounds in 1.250 seconds)
Target 1 (mutex linked list):
mean of 50152567.538 r/s (62694400 rounds in 1.250 seconds)
Target 2 (lockfree):
mean of 25788471.153 r/s (32237568 rounds in 1.250 seconds)

Result for 2 threads:
Target 0 (mutex vector):
mean of 10269221.414 r/s (12838912 rounds in 1.250 seconds)
Target 1 (mutex linked list):
mean of 15027960.930 r/s (18787328 rounds in 1.250 seconds)
Target 2 (lockfree):
mean of 6761175.851 r/s (8452096 rounds in 1.250 seconds)

Result for 4 threads:
Target 0 (mutex vector):
mean of 18438887.804 r/s (23052288 rounds in 1.250 seconds)
Target 1 (mutex linked list):
mean of 11941267.117 r/s (14928896 rounds in 1.250 seconds)
Target 2 (lockfree):
mean of 5866365.571 r/s (7334912 rounds in 1.250 seconds)

Result for 8 threads:
Target 0 (mutex vector):
mean of 10719935.758 r/s (13403136 rounds in 1.250 seconds)
Target 1 (mutex linked list):
mean of 5805096.283 r/s (7259136 rounds in 1.250 seconds)
Target 2 (lockfree):
mean of 5299406.812 r/s (6633472 rounds in 1.252 seconds)

Result for 16 threads:
Target 0 (mutex vector):
mean of 11043957.290 r/s (13813760 rounds in 1.251 seconds)
Target 1 (mutex linked list):
mean of 6037878.001 r/s (7555072 rounds in 1.251 seconds)
Target 2 (lockfree):
mean of 4700504.434 r/s (5966848 rounds in 1.269 seconds)

```

## STACK
```

Result for 1 threads:
Target 0 (mutex vector):
mean of 93598777.390 r/s (117006336 rounds in 1.250 seconds)
Target 1 (mutex linked list):
mean of 50316733.711 r/s (62900224 rounds in 1.250 seconds)
Target 2 (lockfree):
mean of 32980961.200 r/s (41229312 rounds in 1.250 seconds)

Result for 2 threads:
Target 0 (mutex vector):
mean of 16577396.180 r/s (20723712 rounds in 1.250 seconds)
Target 1 (mutex linked list):
mean of 15416987.390 r/s (19272704 rounds in 1.250 seconds)
Target 2 (lockfree):
mean of 7796934.280 r/s (9748480 rounds in 1.250 seconds)

Result for 4 threads:
Target 0 (mutex vector):
mean of 20578554.254 r/s (25726976 rounds in 1.250 seconds)
Target 1 (mutex linked list):
mean of 13290525.384 r/s (16615424 rounds in 1.250 seconds)
Target 2 (lockfree):
mean of 6550535.360 r/s (8190976 rounds in 1.250 seconds)

Result for 8 threads:
Target 0 (mutex vector):
mean of 11936346.837 r/s (14925824 rounds in 1.250 seconds)
Target 1 (mutex linked list):
mean of 5336906.024 r/s (6673408 rounds in 1.250 seconds)
Target 2 (lockfree):
mean of 6232121.974 r/s (7796736 rounds in 1.251 seconds)

Result for 16 threads:
Target 0 (mutex vector):
mean of 11595009.037 r/s (14504960 rounds in 1.251 seconds)
Target 1 (mutex linked list):
mean of 6157158.225 r/s (7704576 rounds in 1.251 seconds)
Target 2 (lockfree):
mean of 5123655.888 r/s (6430720 rounds in 1.255 seconds)

```

## MAP
```
Result for 1 threads:
Target 0 (mutex insert):
mean of 10846880.976 r/s (13559808 rounds in 1.250 seconds)
Target 1 (lockfree insert):
mean of 3083291.818 r/s (3855360 rounds in 1.250 seconds)

Result for 2 threads:
Target 0 (mutex insert):
mean of 5545038.052 r/s (6932480 rounds in 1.250 seconds)
Target 1 (lockfree insert):
mean of 4706173.460 r/s (5986304 rounds in 1.272 seconds)

Result for 4 threads:
Target 0 (mutex insert):
mean of 5346668.400 r/s (6686720 rounds in 1.251 seconds)
Target 1 (lockfree insert):
mean of 4469036.132 r/s (5588992 rounds in 1.251 seconds)

Result for 8 threads:
Target 0 (mutex insert):
mean of 4228601.332 r/s (5289984 rounds in 1.251 seconds)
Target 1 (lockfree insert):
mean of 4485519.649 r/s (5612544 rounds in 1.251 seconds)

Result for 1 threads:
Target 0 (mutex get):
mean of 8456796.039 r/s (10571776 rounds in 1.250 seconds)
Target 1 (lockfree get):
mean of 5186747.966 r/s (6483968 rounds in 1.250 seconds)

Result for 2 threads:
Target 0 (mutex get):
mean of 5509905.538 r/s (6889472 rounds in 1.250 seconds)
Target 1 (lockfree get):
mean of 8401315.266 r/s (10504192 rounds in 1.250 seconds)

Result for 4 threads:
Target 0 (mutex get):
mean of 5940744.451 r/s (7427072 rounds in 1.250 seconds)
Target 1 (lockfree get):
mean of 11178689.147 r/s (13976576 rounds in 1.250 seconds)

Result for 8 threads:
Target 0 (mutex get):
mean of 4668769.695 r/s (5840896 rounds in 1.251 seconds)
Target 1 (lockfree get):
mean of 12274648.553 r/s (15347712 rounds in 1.250 seconds)

Result for 1 threads:
Target 0 (mutex remove):
mean of 9078530.816 r/s (11348992 rounds in 1.250 seconds)
Target 1 (lockfree remove):
mean of 2963957.668 r/s (3705856 rounds in 1.250 seconds)

Result for 2 threads:
Target 0 (mutex remove):
mean of 8176245.955 r/s (10221568 rounds in 1.250 seconds)
Target 1 (lockfree remove):
mean of 13052455.433 r/s (16318464 rounds in 1.250 seconds)

Result for 4 threads:
Target 0 (mutex remove):
mean of 6763708.250 r/s (8456192 rounds in 1.250 seconds)
Target 1 (lockfree remove):
mean of 13651569.473 r/s (17068032 rounds in 1.250 seconds)

Result for 8 threads:
Target 0 (mutex remove):
mean of 6107615.974 r/s (7639040 rounds in 1.251 seconds)
Target 1 (lockfree remove):
mean of 11677936.452 r/s (14601216 rounds in 1.250 seconds)

Result for 1 threads:
Target 0 (mutex mixed):
mean of 8923787.074 r/s (11155456 rounds in 1.250 seconds)
Target 1 (lockfree mixed):
mean of 3198851.262 r/s (3999744 rounds in 1.250 seconds)

Result for 2 threads:
Target 0 (mutex mixed):
mean of 4703759.900 r/s (5880832 rounds in 1.250 seconds)
Target 1 (lockfree mixed):
mean of 2210759.001 r/s (2764800 rounds in 1.251 seconds)

Result for 4 threads:
Target 0 (mutex mixed):
mean of 4431257.942 r/s (5541888 rounds in 1.251 seconds)
Target 1 (lockfree mixed):
mean of 2673543.465 r/s (3344384 rounds in 1.251 seconds)

Result for 8 threads:
Target 0 (mutex mixed):
mean of 3281505.872 r/s (4107264 rounds in 1.252 seconds)
Target 1 (lockfree mixed):
mean of 2563777.779 r/s (3209216 rounds in 1.252 seconds)

```

## MPSC CHANNEL
```
Mutexed VecDeque with 3 threads total time: 99.229711ms
Std's MPSC with 3 threads total time: 37.291016ms
Lockfree MPSC with 3 threads total time: 78.990366ms

Mutexed VecDeque with 5 threads total time: 250.676698ms
Std's MPSC with 5 threads total time: 165.328736ms
Lockfree MPSC with 5 threads total time: 174.561518ms

Mutexed VecDeque with 9 threads total time: 533.562317ms
Std's MPSC with 9 threads total time: 449.719126ms
Lockfree MPSC with 9 threads total time: 406.557324ms

Mutexed VecDeque with 17 threads total time: 972.406324ms
Std's MPSC with 17 threads total time: 984.550523ms
Lockfree MPSC with 17 threads total time: 1.406755476s

Mutexed VecDeque with 33 threads total time: 1.996519495s
Std's MPSC with 33 threads total time: 1.97039955s
Lockfree MPSC with 33 threads total time: 2.491584752s
```

## SPSC CHANNEL
```
Mutexed VecDeque total time: 280.167002ms
Std's MPSC (as SPSC) total time: 56.16794ms
Lockfree SPSC total time: 288.539261ms
```

## SPMC CHANNEL
```
Mutexed VecDeque with 3 threads total time: 77.228983ms
Mutexed Std's MPSC (as SPMC) with 3 threads total time: 45.99267ms
Lockfree SPMC with 3 threads total time: 96.200218ms

Mutexed VecDeque with 5 threads total time: 226.588922ms
Mutexed Std's MPSC (as SPMC) with 5 threads total time: 70.179382ms
Lockfree SPMC with 5 threads total time: 85.865068ms

Mutexed VecDeque with 9 threads total time: 450.579857ms
Mutexed Std's MPSC (as SPMC) with 9 threads total time: 130.481769ms
Lockfree SPMC with 9 threads total time: 114.333799ms

Mutexed VecDeque with 17 threads total time: 815.07391ms
Mutexed Std's MPSC (as SPMC) with 17 threads total time: 125.530757ms
Lockfree SPMC with 17 threads total time: 133.102409ms

Mutexed VecDeque with 33 threads total time: 1.618507497s
Mutexed Std's MPSC (as SPMC) with 33 threads total time: 133.219862ms
Lockfree SPMC with 33 threads total time: 142.728936ms
```

## MPMC CHANNEL
```
Mutexed VecDeque with 4 threads total time: 44.44874ms
Mutexed Std's MPSC (as MPMC)  with 4 threads total time: 24.819183ms
Lockfree MPMC with 4 threads total time: 38.809402ms

Mutexed VecDeque with 8 threads total time: 127.893584ms
Mutexed Std's MPSC (as MPMC)  with 8 threads total time: 69.969399ms
Lockfree MPMC with 8 threads total time: 96.48539ms

Mutexed VecDeque with 16 threads total time: 241.13194ms
Mutexed Std's MPSC (as MPMC)  with 16 threads total time: 259.731871ms
Lockfree MPMC with 16 threads total time: 221.155085ms
```

## SKIPLIST
```
Result for 1 threads:
Target 0 (mutex btree_map insert):
mean of 33394591.582 r/s (41746432 rounds in 1.250 seconds)
Target 1 (lockfree insert):
mean of 535996.169 r/s (670720 rounds in 1.251 seconds)

Result for 2 threads:
Target 0 (mutex btree_map insert):
mean of 9779213.134 r/s (12225536 rounds in 1.250 seconds)
Target 1 (lockfree insert):
mean of 122675.592 r/s (154624 rounds in 1.260 seconds)

Result for 4 threads:
Target 0 (mutex btree_map insert):
mean of 9550015.484 r/s (11940864 rounds in 1.250 seconds)
Target 1 (lockfree insert):
mean of 87827.288 r/s (111616 rounds in 1.271 seconds)

Result for 8 threads:
Target 0 (mutex btree_map insert):
mean of 5873525.523 r/s (7346176 rounds in 1.251 seconds)
Target 1 (lockfree insert):
mean of 74706.574 r/s (97280 rounds in 1.302 seconds)

Result for 16 threads:
Target 0 (mutex btree_map insert):
mean of 6428722.881 r/s (8048640 rounds in 1.252 seconds)
Target 1 (lockfree insert):
mean of 66287.957 r/s (92160 rounds in 1.390 seconds)

Result for 32 threads:
Target 0 (mutex btree_map insert):
mean of 6397300.717 r/s (8016896 rounds in 1.253 seconds)
Target 1 (lockfree insert):
mean of 66126.691 r/s (97280 rounds in 1.471 seconds)

Result for 1 threads:
Target 0 (mutex btree_map get):
mean of 33017271.809 r/s (41274368 rounds in 1.250 seconds)
Target 1 (lockfree get):
mean of 1179084.029 r/s (1474560 rounds in 1.251 seconds)

Result for 2 threads:
Target 0 (mutex btree_map get):
mean of 9267551.222 r/s (11585536 rounds in 1.250 seconds)
Target 1 (lockfree get):
mean of 215240.748 r/s (269312 rounds in 1.251 seconds)

Result for 4 threads:
Target 0 (mutex btree_map get):
mean of 10536458.831 r/s (13172736 rounds in 1.250 seconds)
Target 1 (lockfree get):
mean of 171996.438 r/s (216064 rounds in 1.256 seconds)

Result for 8 threads:
Target 0 (mutex btree_map get):
mean of 8212504.786 r/s (10269696 rounds in 1.250 seconds)
Target 1 (lockfree get):
mean of 147577.612 r/s (188416 rounds in 1.277 seconds)

Result for 16 threads:
Target 0 (mutex btree_map get):
mean of 7855815.680 r/s (9828352 rounds in 1.251 seconds)
Target 1 (lockfree get):
mean of 129797.793 r/s (171008 rounds in 1.317 seconds)

Result for 32 threads:
Target 0 (mutex btree_map get):
mean of 7839423.390 r/s (9816064 rounds in 1.252 seconds)
Target 1 (lockfree get):
mean of 129421.077 r/s (176128 rounds in 1.361 seconds)

Result for 1 threads:
Target 0 (mutex btree_map pop_first):
mean of 93689253.493 r/s (117118976 rounds in 1.250 seconds)
Target 1 (lockfree get pop_first):
mean of 35461165.484 r/s (44329984 rounds in 1.250 seconds)

Result for 2 threads:
Target 0 (mutex btree_map pop_first):
mean of 20534562.895 r/s (25670656 rounds in 1.250 seconds)
Target 1 (lockfree get pop_first):
mean of 6837216.726 r/s (8547328 rounds in 1.250 seconds)

Result for 4 threads:
Target 0 (mutex btree_map pop_first):
mean of 19646908.862 r/s (24561664 rounds in 1.250 seconds)
Target 1 (lockfree get pop_first):
mean of 5247058.236 r/s (6561792 rounds in 1.251 seconds)

Result for 8 threads:
Target 0 (mutex btree_map pop_first):
mean of 15501697.026 r/s (19382272 rounds in 1.250 seconds)
Target 1 (lockfree get pop_first):
mean of 4519344.860 r/s (5653504 rounds in 1.251 seconds)

Result for 16 threads:
Target 0 (mutex btree_map pop_first):
mean of 16499415.992 r/s (20634624 rounds in 1.251 seconds)
Target 1 (lockfree get pop_first):
mean of 4097288.540 r/s (5129216 rounds in 1.252 seconds)

Result for 32 threads:
Target 0 (mutex btree_map pop_first):
mean of 16703976.673 r/s (20898816 rounds in 1.251 seconds)
Target 1 (lockfree get pop_first):
mean of 4103046.322 r/s (5147648 rounds in 1.255 seconds)

```

