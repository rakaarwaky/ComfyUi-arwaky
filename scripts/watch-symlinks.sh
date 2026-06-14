#!/bin/bash

TARGET_DIR="/home/raka/App/ComfyUi-arwaky/ComfyUI"
SHARED_DIR="/home/raka/SharedData"

restore_symlinks() {
    for item in input models output user; do
        path="$TARGET_DIR/$item"
        target="$SHARED_DIR/${item^}" 
        
        # Jika bukan symlink (bisa jadi terhapus atau berubah jadi folder biasa)
        if [ ! -L "$path" ]; then
            echo "$(date): Memulihkan symlink untuk $item -> $target"
            rm -rf "$path"
            ln -s "$target" "$path"
        fi
    done
}

# Jalankan sekali saat startup
restore_symlinks

# Pantau perubahan pembuatan/penghapusan file/folder di dalam ComfyUI
inotifywait -m -e create -e delete -e move -e delete_self "$TARGET_DIR" | while read -r _directory _event file; do
    if [[ "$file" =~ ^(input|models|output|user)$ ]]; then
        # Beri jeda 1 detik agar proses Git menyelesaikan operasinya terlebih dahulu
        sleep 1
        restore_symlinks
    fi
done
