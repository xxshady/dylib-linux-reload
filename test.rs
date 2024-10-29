fn mut_() {
  let mut x = &mut 42;
  let mut f = || {
      *(*&mut x) += 27;
  };
  &x;
  f();
}

fn unique() {
  let x = &mut 42;
  let mut f = || {
      *x += 27;
  };
  &x;
  f();
}

fn main() {}
