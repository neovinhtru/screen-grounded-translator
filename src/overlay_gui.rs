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
                            app.selection_rect = None;
                            app.translation_result = None;
                            return;
                        }

                        // Draw Dimming Overlay
                        let screen_rect_local = ctx.screen_rect(); // This is 0,0 to w,h
                        let painter = ui.painter();
                        let dim_color = egui::Color32::from_black_alpha(100);
                        
                        // Calculate global selection rect
                        let global_selection = app.selection_rect;

                        if let Some(global_rect) = global_selection {
                            // Convert global rect to local coordinates for this screen
                            let local_selection = global_rect.translate(egui::vec2(
                                -screen.x as f32,
                                -screen.y as f32
                            ));

                            // Intersect with this screen's bounds to see if we need to draw a hole
                            // Actually, we can just draw the 4 rectangles around the local_selection
                            // relative to screen_rect_local.
                            
                            // We need to clamp the local_selection to the screen_rect_local for the "hole" logic to work visually?
                            // Or just draw the 4 rects extending to infinity (or screen bounds).
                            
                            let top_rect = egui::Rect::from_min_max(
                                screen_rect_local.min,
                                egui::pos2(screen_rect_local.max.x, local_selection.min.y)
                            );
                            let bottom_rect = egui::Rect::from_min_max(
                                egui::pos2(screen_rect_local.min.x, local_selection.max.y),
                                screen_rect_local.max
                            );
                            let left_rect = egui::Rect::from_min_max(
                                egui::pos2(screen_rect_local.min.x, local_selection.min.y),
                                egui::pos2(local_selection.min.x, local_selection.max.y)
                            );
                            let right_rect = egui::Rect::from_min_max(
                                egui::pos2(local_selection.max.x, local_selection.min.y),
                                egui::pos2(screen_rect_local.max.x, local_selection.max.y)
                            );

                            // Only draw if valid (min < max)
                            if top_rect.min.y < top_rect.max.y { painter.rect_filled(top_rect, 0.0, dim_color); }
                            if bottom_rect.min.y < bottom_rect.max.y { painter.rect_filled(bottom_rect, 0.0, dim_color); }
                            if left_rect.min.x < left_rect.max.x { painter.rect_filled(left_rect, 0.0, dim_color); }
                            if right_rect.min.x < right_rect.max.x { painter.rect_filled(right_rect, 0.0, dim_color); }

                            // Draw Selection Border
                            painter.rect_stroke(
                                local_selection,
                                0.0,
                                egui::Stroke::new(2.0, egui::Color32::GREEN),
                            );

                        } else {
                            // No selection, dim the whole screen
                            painter.rect_filled(screen_rect_local, 0.0, dim_color);
                        }

                        // Draw Result (Only on the screen where the selection ends?)
                        // Or just draw it on the screen that contains the center of selection?
                        let result_text = app.translation_result.clone();
                        if let Some(text) = result_text {
                            if let Some(global_rect) = app.selection_rect {
                                // Check if this screen contains the result position (bottom-left of selection)
                                let result_pos = global_rect.max; // Bottom-right actually
                                
                                // Check if result_pos is within this screen's bounds
                                let screen_min_x = screen.x as f32;
                                let screen_min_y = screen.y as f32;
                                let screen_max_x = screen_min_x + screen.width_logical as f32;
                                let screen_max_y = screen_min_y + screen.height_logical as f32;
                                
                                if result_pos.x >= screen_min_x && result_pos.x < screen_max_x &&
                                   result_pos.y >= screen_min_y && result_pos.y < screen_max_y {
                                       
                                     let local_result_pos = result_pos - egui::vec2(screen_min_x, screen_min_y);
                                     
                                     egui::Window::new("Result")
                                        .frame(egui::Frame::window(&ctx.style())
                                            .fill(egui::Color32::from_rgb(30, 30, 30)) // Dark background
                                            .rounding(egui::Rounding::same(8.0))
                                            .shadow(eframe::epaint::Shadow::small_dark())
                                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(60)))
                                        )
                                        .title_bar(false)
                                        .default_pos(local_result_pos)
                                        .resizable(false)
                                        .show(ctx, |ui| {
                                            ui.set_max_width(300.0);
                                            
                                            // Header
                                            ui.horizontal(|ui| {
                                                ui.label(egui::RichText::new("Translation Result").strong().color(egui::Color32::WHITE));
                                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                    if ui.button("❌").clicked() {
                                                        app.show_overlay = false;
                                                        app.selection_rect = None;
                                                        app.translation_result = None;
                                                    }
                                                });
                                            });
                                            
                                            ui.separator();
                                            
                                            // Content
                                            egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                                                ui.add(egui::Label::new(egui::RichText::new(&text).color(egui::Color32::from_gray(220))).selectable(true));
                                            });
                                            
                                            ui.add_space(5.0);
                                            
                                            // Actions
                                            if !text.trim().is_empty() {
                                                ui.horizontal(|ui| {
                                                    let mut show_copied_feedback = false;
                                                    if let Some(last_copy) = app.copy_feedback_time {
                                                        if last_copy.elapsed().as_secs() < 2 {
                                                            show_copied_feedback = true;
                                                        }
                                                    }

                                                    if show_copied_feedback {
                                                        ui.label(egui::RichText::new("✅ Copied!").color(egui::Color32::GREEN));
                                                    } else {
                                                        if ui.button("📋 Copy to Clipboard").clicked() {
                                                            // Use native macOS clipboard
                                                            match crate::platform::macos::copy_to_clipboard(&text) {
                                                                Ok(_) => {
                                                                    app.copy_feedback_time = Some(std::time::Instant::now());
                                                                },
                                                                Err(e) => {
                                                                    eprintln!("Error: [Overlay] Native copy failed: {}", e);
                                                                },
                                                            }
                                                        }
                                                    }
                                                });
                                            }
                                        });
                                }
                            }
                        }

                        // Selection Logic
                        let pointer_pos = ctx.input(|i| i.pointer.interact_pos());
                        
                        if !ctx.wants_pointer_input() {
                            if ctx.input(|i| i.pointer.primary_pressed()) {
                                if let Some(pos) = pointer_pos {
                                    // Convert local pos to global
                                    let global_pos = pos + egui::vec2(screen.x as f32, screen.y as f32);
                                    app.selection_rect = Some(egui::Rect::from_min_max(global_pos, global_pos));
                                    app.translation_result = None;
                                }
                            }
                        }
                        
                        if ctx.input(|i| i.pointer.primary_down()) {
                            if let Some(pos) = pointer_pos {
                                let global_pos = pos + egui::vec2(screen.x as f32, screen.y as f32);
                                // Only update if we are NOT showing a result (meaning we are selecting)
                                let is_selecting = app.translation_result.is_none();
                                if is_selecting {
                                    if let Some(rect) = &mut app.selection_rect {
                                        rect.max = global_pos;
                                    }
                                }
                            }
                        }

                        if ctx.input(|i| i.pointer.primary_released()) {
                            if let Some(rect) = app.selection_rect {
                                // Only capture if we are NOT showing a result
                                if app.translation_result.is_none() {
                                    // Normalize rect
                                    let min = egui::pos2(rect.min.x.min(rect.max.x), rect.min.y.min(rect.max.y));
                                    let max = egui::pos2(rect.min.x.max(rect.max.x), rect.min.y.max(rect.max.y));
                                    let norm_rect = egui::Rect::from_min_max(min, max);
                                    
                                    if norm_rect.width() > 10.0 && norm_rect.height() > 10.0 {
                                            // Trigger capture and translation
                                            let app_clone = APP.clone();
                                            let rect_clone = norm_rect;
                                            
                                            // Hide overlay immediately
                                            app.show_overlay = false; 
                                            
                                            std::thread::spawn(move || {
                                                std::thread::sleep(std::time::Duration::from_millis(150));
                                                
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
                                                                guard.translation_result = Some(text);
                                                                guard.show_overlay = true;
                                                            }
                                                            Err(e) => {
                                                                let mut guard = app_clone.lock().unwrap();
                                                                guard.translation_result = Some(format!("Error: {}", e));
                                                                guard.show_overlay = true;
                                                            }
                                                        }
                                                    }
                                                    Err(e) => {
                                                        println!("Capture failed: {}", e);
                                                        let mut guard = app_clone.lock().unwrap();
                                                        guard.show_overlay = true;
                                                    }
                                                }
                                                
                                                if let Some(ctx) = &app_clone.lock().unwrap().egui_ctx {
                                                    ctx.request_repaint();
                                                }
                                            });
                                    } else {
                                        app.selection_rect = None;
                                    }
                                }
                            }
                        }
                        

                    });
            }
        );
    }
}
