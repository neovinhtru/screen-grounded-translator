# Hướng dẫn chạy Screen Grounded Translator trên macOS

Dự án đã được cập nhật để hỗ trợ macOS (Apple Silicon). Dưới đây là các bước để build và chạy ứng dụng.

## Yêu cầu hệ thống

*   **macOS**: Phiên bản 10.15 trở lên (khuyên dùng macOS 14+ trên Apple Silicon).
*   **Rust**: Đã cài đặt `rustc` và `cargo`. Nếu chưa có, cài đặt qua: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`

## Cách Build và Chạy

1.  **Mở Terminal** tại thư mục dự án:
    ```bash
    cd /Volumes/Datas/www/neo/screen-grounded-translator
    ```

2.  **Build ứng dụng** (chế độ release để tối ưu hiệu năng):
    ```bash
    cargo build --release
    ```

3.  **Chạy ứng dụng**:
    ```bash
    ./target/release/screen-grounded-translator
    ```

## Cấp quyền (Quan trọng)

Khi chạy lần đầu, macOS sẽ yêu cầu các quyền sau:

1.  **Screen Recording (Ghi màn hình)**: Để ứng dụng có thể chụp ảnh màn hình và lấy văn bản.
    *   Vào *System Settings* > *Privacy & Security* > *Screen Recording*.
    *   Bật switch cho `screen-grounded-translator` (hoặc `Terminal` nếu bạn chạy từ terminal).
2.  **Accessibility (Trợ năng)**: Để ứng dụng có thể bắt phím tắt toàn cục (Global Hotkey).
    *   Vào *System Settings* > *Privacy & Security* > *Accessibility*.
    *   Bật switch cho ứng dụng.

**Lưu ý:** Sau khi cấp quyền, bạn có thể cần khởi động lại ứng dụng để quyền có hiệu lực.

## Cách sử dụng

*   **Phím tắt mặc định**: Phím `~` (ngay dưới phím Esc).
*   **Dịch**: Nhấn `~`, màn hình sẽ tối đi. Kéo chuột để chọn vùng văn bản cần dịch. Kết quả sẽ hiện ra ngay lập tức.
*   **Cài đặt**: Ứng dụng chạy ngầm dưới khay hệ thống (System Tray) trên thanh Menu Bar. Click vào icon để mở cài đặt hoặc thoát.
