package com.tapsend.android;

import android.app.job.JobInfo;
import android.app.job.JobParameters;
import android.app.job.JobScheduler;
import android.app.job.JobService;
import android.content.ComponentName;
import android.content.Context;
import android.os.Handler;
import android.os.Looper;

import org.json.JSONObject;

import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

public final class PasteDelivery extends JobService {
    private static final int JOB_ID = 0x545350;
    private final ExecutorService executor = Executors.newSingleThreadExecutor();
    private volatile JobParameters active;
    private volatile TapSendClient client;

    static boolean schedule(Context context, boolean newItems) {
        JobScheduler jobs = context.getSystemService(JobScheduler.class);
        if (!newItems && jobs.getPendingJob(JOB_ID) != null) return true;
        JobInfo request = new JobInfo.Builder(JOB_ID, new ComponentName(context, PasteDelivery.class))
                .setRequiredNetworkType(JobInfo.NETWORK_TYPE_ANY)
                .setBackoffCriteria(10_000, JobInfo.BACKOFF_POLICY_EXPONENTIAL)
                .setPersisted(true)
                .build();
        return jobs.schedule(request) == JobScheduler.RESULT_SUCCESS;
    }

    @Override public boolean onStartJob(JobParameters params) {
        active = params;
        executor.execute(() -> {
            boolean retry = false;
            try {
                if (active == params) deliver(params);
            } catch (Exception error) {
                retry = true;
            }
            boolean needsRetry = retry;
            new Handler(Looper.getMainLooper()).post(() -> {
                if (active == params) {
                    active = null;
                    jobFinished(params, needsRetry);
                }
            });
        });
        return true;
    }

    private void deliver(JobParameters params) throws Exception {
        IdentityStore store = new IdentityStore(this);
        IdentityStore.Identity identity = store.load();
        if (identity == null) return;
        File[] batches = PasteOutbox.batches(this, identity.deviceId);
        if (batches.length == 0) return;
        TapSendClient recipient = new TapSendClient(store, params.getNetwork());
        client = recipient;
        try {
            recipient.loadAndAuthenticate();
            if (!identity.deviceId.equals(recipient.identityId())) return;
            for (File batch : batches) {
                for (File file : PasteOutbox.entries(batch)) {
                    if (active != params) return;
                    JSONObject entry = new JSONObject(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8));
                    File payload = PasteOutbox.payload(file);
                    recipient.sendPaste(entry.getString("id"), entry.getString("kind"), entry.getString("label"), payload);
                    Files.delete(file.toPath());
                    payload.delete();
                }
                PasteOutbox.deleteBatch(batch);
            }
        } finally {
            client = null;
        }
    }

    @Override public boolean onStopJob(JobParameters params) {
        if (active == params) {
            active = null;
            TapSendClient sending = client;
            if (sending != null) sending.cancelTransfer();
        }
        return true;
    }

    @Override public void onDestroy() {
        active = null;
        TapSendClient sending = client;
        if (sending != null) sending.cancelTransfer();
        executor.shutdownNow();
        super.onDestroy();
    }
}
