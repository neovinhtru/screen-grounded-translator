use eframe::egui;
use crate::APP;
use crate::api::translate_image;


pub fn show_overlay(ctx: &egui::Context) {
    let screens = {
        let app = APP.lock().unwrap();
        app.screens.clone()
    };

    if screens.is_empty() {
        return;
    }

    for (index, screen) in screens.iter().enumerate() {
        let title = format!("Screen Grounded Translator Overlay {}", index);
        let viewport_id = egui::ViewportId::from_hash_of(&title);
        
        ctx.show_viewport_immediate(
            viewport_id,
            egui::ViewportBuilder::default()
                .with_title(&title)
                .with_inner_size([screen.width_logical as f32, screen.height_logical as f32])
                .with_position(egui::pos2(screen.x as f32, screen.y as f32))
                .with_transparent(true)
                .with_decorations(false)
                .with_always_on_top(),
            |ctx, _class| {
                // Force clear color to be transparent
                ctx.set_visuals(egui::Visuals {
                    window_fill: egui::Color32::TRANSPARENT,
                    panel_fill: egui::Color32::TRANSPARENT,
                    ..Default::default()
                });
                
                // Force macOS window transparency
                #[cfg(target_os = "macos")]
                crate::platform::macos::make_window_transparent(&title);

                egui::CentralPanel::default()
                    .frame(egui::Frame::none().fill(egui::Color32::TRANSPARENT))
                    .show(ctx, |ui| {
                        let mut app = APP.lock().unwrap();
                        
                        // Handle Escape to close
                        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                            app.show_overlay = false;
                            app.current_selection = None;
                            app.translation_items.clear();
                            return;
                        }

                        // Draw Dimming Overlay
                        let screen_rect_local = ctx.screen_rect(); // This is 0,0 to w,h
                        let painter = ui.painter();
                        let dim_color = egui::Color32::from_black_alpha(100);
                        
                        // We want to dim everything EXCEPT the current selection and existing result areas.
                        // However, complex subtraction is hard. Simple approach: dim everything, 
                        // and draw "holes" or just draw the selection border on top.
                        // For now, let's keep the simple dimming of the whole screen, 
                        // but maybe we can make the selection area clear?
                        // Actually, drawing a full screen dim rect is fine, as long as we draw results on top.
                        painter.rect_filled(screen_rect_local, 0.0, dim_color);

                        // Draw Current Selection (Drag)
                        if let Some(global_rect) = app.current_selection {
                             let local_selection = global_rect.translate(egui::vec2(
                                -screen.x as f32,
                                -screen.y as f32
                            ));
                            
                            // Draw clear hole for selection? Or just a border?
                            // Let's draw a border and a lighter fill
                            painter.rect_stroke(
                                local_selection,
                                0.0,
                                egui::Stroke::new(2.0, egui::Color32::WHITE)
                            );
                            painter.rect_filled(
                                local_selection,
                                0.0,
                                egui::Color32::from_white_alpha(20)
                            );
                        }

                        // Draw Translation Items (Result Windows)
                        // We need to collect IDs first to avoid borrowing issues when modifying app
                        let item_ids: Vec<usize> = app.translation_items.iter().map(|i| i.id).collect();
                        let mut items_to_remove = Vec::new();
                        let mut copy_action = None; // (text, item_id)

                        for id in item_ids {
                            if let Some(item) = app.translation_items.iter_mut().find(|i| i.id == id) {
                                // Check if this item belongs to this screen (or overlaps significantly)
                                // For simplicity, we render it if it intersects, or just render it relative to this screen.
                                // Since we have multiple viewports covering the whole virtual space, 
                                // we should only render if the top-left of the result is on this screen?
                                // Or just render and let the OS clip it? 
                                // Egui viewports are separate OS windows. We need to render content in the correct viewport.
                                // Let's calculate local rect.
                                let local_rect = item.selection_rect.translate(egui::vec2(
                                    -screen.x as f32,
                                    -screen.y as f32
                                ));

                                // Only render if it's somewhat visible on this screen
                                if local_rect.intersects(screen_rect_local) {
                                    // Draw selection border for this item
                                    painter.rect_stroke(
                                        local_rect,
                                        0.0,
                                        egui::Stroke::new(2.0, egui::Color32::GREEN)
                                    );

                                    // Draw Result Window
                                    // Position it below the selection
                                    let window_pos = local_rect.max + egui::vec2(0.0, 10.0);
                                    
                                    let mut is_open = true;
                                    let window_title = match item.status {
                                        crate::TranslationStatus::Translating => "Translating...",
                                        crate::TranslationStatus::Success => "Translation",
                                        crate::TranslationStatus::Error => "Error",
                                    };

                                    egui::Window::new(window_title)
                                        .id(egui::Id::new(item.id))
                                        .default_pos(window_pos)
                                        .open(&mut is_open)
                                        .frame(egui::Frame::window(&ctx.style())
                                            .fill(egui::Color32::from_black_alpha(220))
                                            .rounding(8.0)
                                            .shadow(eframe::epaint::Shadow::small_dark())
                                            .inner_margin(12.0)
                                        )
                                        .show(ctx, |ui| {
                                            ui.set_max_width(400.0);
                                            
                                            // Content
                                            if let crate::TranslationStatus::Translating = item.status {
                                                ui.horizontal(|ui| {
                                                    ui.spinner();
                                                    ui.label("Processing...");
                                                });
                                                ui.add_space(8.0);
                                            }

                                            if !item.text.is_empty() {
                                                ui.add(
                                                    egui::Label::new(&item.text)
                                                        .wrap(true)
                                                        .selectable(true)
                                                );
                                                
                                                ui.add_space(8.0);
                                                
                                                // Copy Button
                                                let show_copied_feedback = if let Some(time) = item.copy_feedback_time {
                                                    time.elapsed().as_secs() < 2
                                                } else {
                                                    false
                                                };

                                                if show_copied_feedback {
                                                    ui.label(egui::RichText::new("✅ Copied!").color(egui::Color32::GREEN));
                                                } else {
                                                    if ui.button("📋 Copy to Clipboard").clicked() {
                                                        copy_action = Some((item.text.clone(), id));
                                                    }
                                                }
                                            }
                                        });
                                    
                                    if !is_open {
                                        items_to_remove.push(id);
                                    }
                                }
                            }
                        }
                        
                        // Apply deferred actions
                        if let Some((text, id)) = copy_action {
                            // Use native macOS clipboard
                            match crate::platform::macos::copy_to_clipboard(&text) {
                                Ok(_) => {
                                    if let Some(item) = app.translation_items.iter_mut().find(|i| i.id == id) {
                                        item.copy_feedback_time = Some(std::time::Instant::now());
                                    }
                                },
                                Err(e) => {
                                    eprintln!("Error: [Overlay] Native copy failed: {}", e);
                                },
                            }
                        }
                        
                        for id in items_to_remove {
                            app.translation_items.retain(|i| i.id != id);
                        }

                        // Selection Logic
                        // We need to handle input relative to global coordinates
                        let pointer_pos = ctx.input(|i| i.pointer.interact_pos());
                        
                        // Only handle selection if we are NOT interacting with a window?
                        // ctx.wants_pointer_input() returns true if a widget is being interacted with.
                        // But we are drawing Areas manually.
                        
                        if !ctx.wants_pointer_input() {
                             if ctx.input(|i| i.pointer.primary_pressed()) {
                                if let Some(pos) = pointer_pos {
                                    // Convert local pos to global
                                    let global_pos = pos + egui::vec2(screen.x as f32, screen.y as f32);
                                    app.current_selection = Some(egui::Rect::from_min_max(global_pos, global_pos));
                                }
                            }
                        }
                        
                        if ctx.input(|i| i.pointer.primary_down()) {
                            if let Some(pos) = pointer_pos {
                                let global_pos = pos + egui::vec2(screen.x as f32, screen.y as f32);
                                if let Some(rect) = &mut app.current_selection {
                                    rect.max = global_pos;
                                }
                            }
                        }

                        if ctx.input(|i| i.pointer.primary_released()) {
                            if let Some(rect) = app.current_selection {
                                // Normalize rect
                                let min = egui::pos2(rect.min.x.min(rect.max.x), rect.min.y.min(rect.max.y));
                                let max = egui::pos2(rect.min.x.max(rect.max.x), rect.min.y.max(rect.max.y));
                                let norm_rect = egui::Rect::from_min_max(min, max);
                                
                                if norm_rect.width() > 10.0 && norm_rect.height() > 10.0 {
                                    // Create new Translation Item
                                    let item_id = app.next_item_id;
                                    app.next_item_id += 1;
                                    
                                    app.translation_items.push(crate::TranslationItem {
                                        id: item_id,
                                        selection_rect: norm_rect,
                                        text: String::new(),
                                        status: crate::TranslationStatus::Translating,
                                        copy_feedback_time: None,
                                    });
                                    
                                    // Trigger capture and translation
                                    let app_clone = APP.clone();
                                    let rect_clone = norm_rect;
                                    
                                    // NOTE: We do NOT hide the overlay anymore!
                                    // app.show_overlay = false; 
                                    
                                    std::thread::spawn(move || {
                                        // Wait a bit? Maybe not needed since we don't hide overlay.
                                        // But we might capture the overlay itself if we are not careful.
                                        // Ideally, we should hide the overlay, capture, then show it again?
                                        // Or use an API that excludes our window.
                                        // `screenshots` crate captures the screen content.
                                        // If our window is transparent and click-through, maybe it's fine?
                                        // But our result windows are opaque.
                                        // We might need to hide overlay briefly.
                                        
                                        // Capture logic starts here
                                        let global_x = rect_clone.min.x as i32;
                                        let global_y = rect_clone.min.y as i32;
                                        let global_w = rect_clone.width() as u32;
                                        let global_h = rect_clone.height() as u32;
                                        
                                        use crate::platform::macos::capture_area;
                                        
                                        match capture_area((global_x, global_y, global_w, global_h)) {
                                            Ok(img) => {
                                                let (config, model) = {
                                                    let mut guard = app_clone.lock().unwrap();
                                                    let model = guard.model_selector.get_next_model();
                                                    (guard.config.clone(), model)
                                                };
                                                
                                                match translate_image(config.api_key, config.target_language, model, img) {
                                                    Ok(text) => {
                                                        let mut guard = app_clone.lock().unwrap();
                                                        if let Some(item) = guard.translation_items.iter_mut().find(|i| i.id == item_id) {
                                                            item.text = text;
                                                            item.status = crate::TranslationStatus::Success;
                                                        }
                                                    },
                                                    Err(e) => {
                                                        let mut guard = app_clone.lock().unwrap();
                                                        if let Some(item) = guard.translation_items.iter_mut().find(|i| i.id == item_id) {
                                                            item.text = format!("Error: {}", e);
                                                            item.status = crate::TranslationStatus::Error;
                                                        }
                                                    }
                                                }
                                            },
                                            Err(e) => {
                                                 let mut guard = app_clone.lock().unwrap();
                                                 if let Some(item) = guard.translation_items.iter_mut().find(|i| i.id == item_id) {
                                                     item.text = format!("Capture Error: {}", e);
                                                     item.status = crate::TranslationStatus::Error;
                                                 }
                                            }
                                        }
                                        
                                        // Request repaint
                                        if let Some(ctx) = &app_clone.lock().unwrap().egui_ctx {
                                            ctx.request_repaint();
                                        }
                                    });
                                }
                                
                                // Clear current selection
                                app.current_selection = None;
                            }
                        }
                    });
            },
        );
    }
}
