# Benchmark

The following benchmark compares `dyn-utils` with alternatives and measures the effect of its different optimizations. It is executed on GitHub runners, so the results should be taken with a grain of salt, but they still give a good approximation.

## Results

### `ubuntu-24.04` (x86-64)[^1]

```
comparison                                      fastest       │ slowest       │ median        │ mean    
├─ async_trait_future                           15.78 ns      │ 517.4 ns      │ 15.87 ns      │ 16.44 ns
├─ dyn_utils_future                             4.714 ns      │ 173.5 ns      │ 4.871 ns      │ 4.942 ns
├─ dyn_utils_future_no_alloc                    4.617 ns      │ 147.3 ns      │ 4.695 ns      │ 4.737 ns
├─ dyn_utils_future_no_alloc_no_drop            3.082 ns      │ 29.21 ns      │ 3.102 ns      │ 3.142 ns
├─ dyn_utils_future_try_sync                    1.918 ns      │ 20.97 ns      │ 1.957 ns      │ 1.986 ns
├─ dyn_utils_future_try_sync_fallback           5.558 ns      │ 111.1 ns      │ 5.617 ns      │ 5.668 ns
├─ dyn_utils_future_with_storage                5.869 ns      │ 101.8 ns      │ 5.91 ns       │ 5.977 ns
├─ dyn_utils_future_with_storage_option_future  5.869 ns      │ 97.03 ns      │ 6.222 ns      │ 6.247 ns
├─ dyn_utils_iter                               15.78 ns      │ 189.3 ns      │ 16.1 ns       │ 16.22 ns
├─ dynify_future                                17.43 ns      │ 288.6 ns      │ 17.51 ns      │ 17.63 ns
├─ dynify_future_no_alloc                       17.11 ns      │ 319.9 ns      │ 17.12 ns      │ 17.3 ns 
├─ dynify_iter                                  16.09 ns      │ 209.2 ns      │ 16.17 ns      │ 16.32 ns
├─ dynosaur_future                              16.41 ns      │ 214.6 ns      │ 16.49 ns      │ 16.62 ns
├─ stackfuture_future_no_alloc                  4.304 ns      │ 39.31 ns      │ 4.324 ns      │ 4.43 ns 
╰─ stackfuture_future_no_alloc_no_drop          4.304 ns      │ 61.28 ns      │ 4.326 ns      │ 4.696 ns
```

### `ubuntu-24.04-arm` (aarch64)[^1]

```
comparison                                      fastest       │ slowest       │ median        │ mean    
├─ async_trait_future                           14.38 ns      │ 236.3 ns      │ 14.61 ns      │ 14.66 ns
├─ dyn_utils_future                             3.632 ns      │ 55.04 ns      │ 3.766 ns      │ 3.775 ns
├─ dyn_utils_future_no_alloc                    2.883 ns      │ 26.49 ns      │ 3.508 ns      │ 3.419 ns
├─ dyn_utils_future_no_alloc_no_drop            2.946 ns      │ 170.8 ns      │ 2.961 ns      │ 2.976 ns
├─ dyn_utils_future_try_sync                    1.332 ns      │ 21.29 ns      │ 1.344 ns      │ 1.362 ns
├─ dyn_utils_future_try_sync_fallback           3.663 ns      │ 43.14 ns      │ 3.711 ns      │ 3.736 ns
├─ dyn_utils_future_with_storage                4.469 ns      │ 92.96 ns      │ 4.672 ns      │ 4.817 ns
├─ dyn_utils_future_with_storage_option_future  4.764 ns      │ 54.68 ns      │ 4.891 ns      │ 4.914 ns
├─ dyn_utils_iter                               6.873 ns      │ 286.1 ns      │ 7.014 ns      │ 7.036 ns
├─ dynify_future                                5.375 ns      │ 94.3 ns       │ 5.891 ns      │ 5.911 ns
├─ dynify_future_no_alloc                       4.623 ns      │ 88.79 ns      │ 4.86 ns       │ 4.895 ns
├─ dynify_iter                                  9.766 ns      │ 194.2 ns      │ 10.11 ns      │ 10.17 ns
├─ dynosaur_future                              14.88 ns      │ 207 ns        │ 15.41 ns      │ 15.46 ns
├─ stackfuture_future_no_alloc                  2.476 ns      │ 44.24 ns      │ 2.563 ns      │ 2.599 ns
╰─ stackfuture_future_no_alloc_no_drop          2.085 ns      │ 71.44 ns      │ 2.234 ns      │ 2.285 ns
```

## Analysis

As expected, allocation-based alternatives are significantly slower. `dynify` does not perform well on x86-64 and is, in any case, behind dyn-utils.

Only `stackfuture` manages to perform slightly better than `dyn-utils`. The reasons behind this difference are likely those mentioned in the [README](../README.md#stackfuture): the lack of drop optimization and the use of an inlined vtable. Notably, `dyn-utils` becomes faster on x86-64 when it can take advantage of the drop optimization.

As also expected, the *maybe_sync* optimization provides a significant performance improvement.

[^1]: https://github.com/wyfo/dyn-utils/actions/runs/20764391881/attempts/2
