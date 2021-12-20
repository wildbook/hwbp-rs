#[macro_export(local_inner_macros)]
macro_rules! multidoc {
    ($(#[$meta:meta])* => $item:item $($items:item)*) => {
        $(#[$meta])*
        $item
        multidoc!($(#[$meta])* => $($items)*);
    };
    ($(#[$meta:meta])* => ) => {}
}
