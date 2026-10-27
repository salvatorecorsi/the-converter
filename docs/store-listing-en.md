# Microsoft Store listing — English (en-US)

Draft for the complete MSIX, to be checked against the final package. Publisher: **CORSIVO**. Developer: **Salvatore Corsi**. The proposed name **The Converter** requires a successful reservation in Partner Center. This listing is translated; the application UI is currently Italian. Do not copy this preamble into public listing fields.

## Short description

Convert files from the Windows 11 context menu with a queue and custom settings. Optional online audio-to-text transcription uses OpenAI and your API key. External engines enable more formats. App UI is Italian.

## Description

Image and image-to-PDF conversion work immediately. Audio/video format conversion requires FFmpeg; PDF export requires Poppler; Office-to-PDF requires LibreOffice. These optional engines are not included. Optional OpenAI audio-to-text transcription requires internet, your API key and separate API billing, not a ChatGPT subscription; it uploads audio to OpenAI. FFmpeg prepares audio that cannot be sent directly. The application UI is Italian.

The Converter brings conversion to the folder where you work. In the Windows 11 context menu, choose Converti in and a compatible format. The job joins a queue with your saved settings, without opening the options window every time.

A placeholder with a pending icon immediately shows where the result will appear. Open it to check progress or cancel the job. When conversion finishes, the completed file replaces the placeholder. Outputs are saved beside their originals by default; numbered names preserve files that already exist.

Built-in conversions cover PNG, JPEG, WebP, BMP, TIFF, GIF and ICO, plus a separate A4 PDF for each source image. Adjust JPEG quality and image width.

With optional FFmpeg, convert audio to MP3, WAV, FLAC, AAC, OGG, Opus and AIFF; convert video to MP4, MKV, MOV, WebM, AVI and animated GIF; or extract its audio. Settings include quality presets, bitrate, sample rate, channels, normalization, video quality, width, frame rate and time trimming.

With Poppler, export PDF pages to PNG/JPEG or extract embedded text. With LibreOffice, convert compatible documents, spreadsheets and presentations to PDF.

Format conversion runs on your PC. Audio is uploaded only when you select OpenAI audio-to-text transcription after enabling it with your API key. Windows Credential Manager protects the key, and you can remove it in settings. Local conversions do not require an in-app account. Preferences, file paths, queue status and error details are stored locally without the key.

Speech transcription creates a TXT file with automatic language detection or an optional language selection. It is not directly available for video files. Review the result because recognition errors are possible. Requests already sent may incur costs even if you cancel the job.

Limitations: OCR, PDF merging and PDF editing are not included. Animated image inputs and multipage TIFF inputs use their first frame or page. Each image produces its own PDF. Office document layout depends on LibreOffice and available fonts. The modern context menu requires Windows 11 x64. The application UI is Italian.

## Product features

Enter each line in a separate feature field, without adding bullet characters:

```text
Converti in command directly in the Windows 11 File Explorer context menu
Quick conversions join a queue without opening the options window each time
Output-folder placeholders with job status available on demand
Convert PNG, JPEG, WebP, BMP, TIFF, GIF and ICO images
Create a separate A4 PDF for each source image
Adjust JPEG quality, image width and output folder
Audio and video conversion with optional user-selected FFmpeg
PDF export with Poppler and Office-to-PDF conversion with optional LibreOffice
Preserve original files and use numbered output names for existing files
Local format conversion; audio is sent to OpenAI only for requested transcription
Optional audio-to-text transcription with your OpenAI API key and separate API charges
```

## Search terms

One term per field; no third-party product names:

```text
file converter
image conversion
image to PDF
context menu
audio conversion
video conversion
conversion queue
```

## Additional system requirements

```text
Windows 11 x64 for the modern context menu
FFmpeg for audio/video; Poppler for PDF export; LibreOffice for Office-to-PDF. Optional engines are not included. The application UI is Italian.
Optional audio-to-text: internet, an OpenAI API key and separately billed API account; FFmpeg when audio preparation is needed. No ChatGPT subscription usage.
```

## What's new

Leave blank for the first Microsoft Store submission. Add release-specific notes for later updates.

## Captions for screenshots to capture

1. Converti in in the Windows 11 context menu. App UI shown in Italian.
2. The queue shows file formats, status and destinations. App UI shown in Italian.
3. Image quality and resizing settings. App UI shown in Italian.
4. Configure optional executables in Strumenti. App UI shown in Italian.

## Fields still required

Public privacy and support URLs, reserved product name, application license and the publisher's verified legal details. Do not claim an open source license until one has been selected and added to the project.

Description: 10,000 characters maximum; short description: 1,000 maximum (this draft is below 270); up to 20 features of 200 characters each; release notes: 1,500 maximum, empty for the first submission. [Official MSIX listing limits](https://learn.microsoft.com/en-us/windows/apps/publish/publish-your-app/msix/add-and-edit-store-listing-info).
