# dylib-linux-reload

Как запустить (только на linux):
`cargo build --all`
`cargo run`

Проблема тред-локалов по-порядку:
1. Деструкторы тред-локалов мешали либе выгружаться (потому что они выполняются только при выходе потока, а главный поток, ну, выходит при закрытии всей программы)
2. Регистрация деструкторов была заменена no-op и либа смогла выгружаться (добавлением флага `RTLD_DEEPBIND` и подменой `__cxa_thread_atexit_impl` в самой либе), но память тред-локалов утекала
3. Из [std](https://github.com/rust-lang/rust/blob/f6e511eec7342f59a25f7c0534f1dbea00d01b14/library/std/src/sys/thread_local/destructors/linux_like.rs#L43) была подсмотрена реализация ручного вызова деструкторов (но только тех деструкторов, которые принадлежат главному потоку, остальные потоки используют оригинальный `__cxa_thread_atexit_impl`)
