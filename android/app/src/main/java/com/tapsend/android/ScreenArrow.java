package com.tapsend.android;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.PixelFormat;
import android.view.View;
import android.view.WindowManager;

final class ScreenArrow extends View {
    private final WindowManager windows;
    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Runnable hide = this::close;
    private final float[] coordinates = new float[4];

    ScreenArrow(Context context) {
        super(context);
        windows = context.getSystemService(WindowManager.class);
        paint.setStrokeCap(Paint.Cap.ROUND);
        paint.setStrokeWidth(4 * getResources().getDisplayMetrics().density);
    }

    void show(String mark) {
        String[] points = mark.split(",", -1);
        if (points.length != coordinates.length) return;
        for (int i = 0; i < points.length; i++) {
            try {
                int value = Integer.parseInt(points[i]);
                if (value < 0 || value > 1000) return;
                coordinates[i] = value / 1000f;
            } catch (NumberFormatException invalid) { return; }
        }
        if (!isAttachedToWindow()) {
            var params = new WindowManager.LayoutParams(-1, -1,
                    WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY,
                    WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE |
                    WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE |
                    WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN,
                    PixelFormat.TRANSLUCENT);
            // Staying below Android's obscuring threshold lets touches pass through.
            params.alpha = 0.78f;
            params.setTitle("TapSend Live Red Pen");
            params.layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_ALWAYS;
            params.setFitInsetsTypes(0);
            try { windows.addView(this, params); }
            catch (SecurityException | WindowManager.BadTokenException denied) { return; }
        }
        removeCallbacks(hide);
        postDelayed(hide, 1600);
        invalidate();
    }

    void close() {
        removeCallbacks(hide);
        if (isAttachedToWindow()) windows.removeViewImmediate(this);
    }

    @Override protected void onDraw(Canvas canvas) {
        float x = coordinates[0] * getWidth(), y = coordinates[1] * getHeight();
        float endX = coordinates[2] * getWidth(), endY = coordinates[3] * getHeight();
        double angle = Math.atan2(endY - y, endX - x);
        float head = Math.max(12, paint.getStrokeWidth() * 4);
        paint.setColor(Color.rgb(255, 36, 48));
        canvas.drawLine(x, y, endX, endY, paint);
        for (double offset : new double[]{-0.55, 0.55}) {
            canvas.drawLine(endX, endY, endX - head * (float)Math.cos(angle + offset),
                    endY - head * (float)Math.sin(angle + offset), paint);
        }
    }
}
