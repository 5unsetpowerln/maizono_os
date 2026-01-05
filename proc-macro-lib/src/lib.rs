use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::token::Unsafe;
use syn::{ItemFn, parse_macro_input};

fn last_path_ident_is(attr: &syn::Attribute, name: &str) -> bool {
    attr.path()
        .segments
        .last()
        .map(|s| s.ident == name)
        .unwrap_or(false)
}

#[proc_macro_attribute]
pub fn align16_fn(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut f = parse_macro_input!(item as ItemFn);

    // #[align16_fn]
    // #[no_mangle]
    // pub unsafe extern "C" fn a(x: u64) -> u64 {
    //     x + 1
    // }
    //
    // 上記のような関数を以下のようなラッパー関数と本体関数に変換するマクロ
    //
    // pub unsafe extern "C" fn __a_inner(x: u64) -> u64 {
    //     x + 1
    // }
    //
    // #[naked]
    // pub unsafe extern "C" fn a(x: u64) -> u64 {
    //     core::arch::naked_asm!(
    //         "mov r11, rsp",
    //         "and rsp, -16",
    //         "sub rsp, 16",
    //         "mov [rsp], r11",
    //         "call {inner}",
    //         "mov r11, [rsp]",
    //         "mov rsp, r11",
    //         "ret",
    //         inner = sym __a_inner,
    // // }

    // naked関数は`#[naked]`+本体が`naked_asm!`のみなので、そもそもそれを満たせない関数を弾く
    if f.sig.asyncness.is_some() {
        return quote!(compile_error!("align16_fn: async fn は非対応")).into();
    }
    if f.sig.constness.is_some() {
        return quote!(compile_error!("align16_fn: const fn は非対応")).into();
    }
    if f.sig.variadic.is_some() {
        return quote!(compile_error!("align16_fn: variadic は非対応")).into();
    }
    if !f.sig.generics.params.is_empty() {
        return quote!(compile_error!("align16_fn: ジェネリクス付き fn は非対応")).into();
    }
    if f.sig.abi.is_none() {
        // naked 関数では Rust ABI は前提にしない（明示 ABI を要求）
        return quote!(compile_error!(
            "stack_align16_call: extern \"C\" 等の ABI 明示が必要"
        ))
        .into();
    }

    // 元の名前と inner 名
    let orig_ident = f.sig.ident.clone();
    let inner_ident = format_ident!("__{}_inner", orig_ident);

    // 関数のもともとの属性を取得する
    let orig_attrs: Vec<syn::Attribute> = f
        .attrs
        .into_iter()
        .filter(|a| !last_path_ident_is(a, "stack_align16_call"))
        .collect();

    // ラッパーの属性: もともとの属性+#[unsafe(naked)]
    let wrapper_attrs = {
        let mut v = Vec::new();
        v.append(&mut orig_attrs.clone());
        v.push(syn::parse_quote!(#[naked]));
        v
    };

    // 本体関数の属性
    let inner_attrs = orig_attrs
        .into_iter()
        // .filter(|a| {
        //     !(last_path_ident_is(a, "no_mangle")
        //         || last_path_ident_is(a, "export_name")
        //         || last_path_ident_is(a, "link_section")
        //         || last_path_ident_is(a, "used")
        //         || last_path_ident_is(a, "unsafe"))
        // })
        .collect::<Vec<_>>();

    f.sig.ident = inner_ident.clone();
    f.attrs = inner_attrs;

    let mut wrapper_sig = f.sig.clone();
    f.sig.abi = Some(syn::parse_quote!(extern "C"));
    wrapper_sig.ident = orig_ident.clone();

    let expanded = quote! {
        #f

        #(#wrapper_attrs)*
        #wrapper_sig {
            unsafe {
                core::arch::naked_asm!(
                    "mov r11, rsp",
                    "and rsp, -0x10", // 0x10*nに揃える
                    "sub rsp, 0x10",
                    "mov [rsp], r11", // もとのrspをメモリに保存
                    "call {inner}", // 本体関数を呼ぶ
                    "mov r11, [rsp]", // rspを復元する
                    "mov rsp, r11",
                    "ret",
                    inner = sym #inner_ident,
                );
            }
        }
    };

    expanded.into()
}
