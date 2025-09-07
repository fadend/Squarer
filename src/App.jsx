import { useState, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import Draggable from 'react-draggable';
import "./App.css";

function App() {
  const nodeRef = useRef(null);
  const [imageDataUri, setImageData] = useState("");
  const [transformedImageDataUri, setTransformedImageData] = useState("");
  const [controlPoints, setControlPoints] = useState([]);
  const [errorMessage, setErrorMessage] = useState("");
  const [imageFile, setImageFile] = useState("");

  // Provided by Gemini
  function arrayBufferToDataURI(arrayBuffer, mimeType) {
    return new Promise((resolve, reject) => {
      const blob = new Blob([arrayBuffer], { type: mimeType });
      const reader = new FileReader();

      reader.onload = (event) => {
        resolve(event.target.result);
      };

      reader.onerror = (error) => {
        reject(error);
      };

      reader.readAsDataURL(blob);
    });
  }

  function clearError() {
    setErrorMessage("");
  }

  async function processImage() {
    const result = await invoke("process_image", { imageDataUri, controlPoints });
    setControlPoints([]);
    const dataUri = await arrayBufferToDataURI(result, "image/jpeg");
    setImageData(dataUri);
    setTransformedImageData(dataUri);
  }

  function imageFileSelected(e) {
    const file = e.target.files[0];
    if (file) {
      setControlPoints([]);
      setImageFile(file.name);
      setTransformedImageData("");
      const reader = new FileReader();
      reader.onload = function (e) {
        setImageData(e.target.result);
      };
      reader.readAsDataURL(file);
    }
  }

  function imageDisplayClick(e) {
    const imageDisplay = document.getElementById("image-display");
    if (e.target !== imageDisplay) {
      return;
    }
    const containerRect = imageDisplay.getBoundingClientRect();
    const xOffset = e.clientX - containerRect.left;
    const yOffset = e.clientY - containerRect.top;
    if (controlPoints.length < 4) {
      controlPoints.push([parseInt(xOffset), parseInt(yOffset)]);
      // Clone the array so that React will recognize the state change.
      setControlPoints([...controlPoints]);
    }
  }

  function handleDrag(e, data) {
    if (typeof e.target.dataset.index === "undefined") {
      console.log("Missing index for drag target");
      return;
    }
    controlPoints[e.target.dataset.index] = [data.x, data.y];
    setControlPoints([...controlPoints]);
  }

  function imageLoaded() {
    const image = document.getElementById("image");
    const imageDisplay = document.getElementById("image-display");
    imageDisplay.style.backgroundImage = `url(${imageDataUri})`;
    imageDisplay.style.width = image.naturalWidth + "px";
    imageDisplay.style.height = image.naturalHeight + "px";
  }

  async function onSubmit(e) {
    e.preventDefault();
    try {
      await processImage();
    } catch (e) {
      setErrorMessage(e.toString());
    }
  }

  return (
    <main className="container">
      <h1>Square up image</h1>
      <form
        onSubmit={onSubmit}
      >
        <label>
          Select an image: <input type="file" accept="image/jpeg" onChange={imageFileSelected} />
        </label>
        <button type="submit" title="Select the 4 corners of the rectangle" disabled={controlPoints.length < 4}>Process</button>
        {
          errorMessage ?
            <div className="error-message">
              <a className="error-clear-button" onClick={clearError}>[x]</a>
              {errorMessage}
            </div>
            : ""
        }
        {
          transformedImageDataUri ?
            <div>
              <a
                href={transformedImageDataUri}
                download={imageFile.replace(/\.[a-z]+$/i, "_squared$&")}>Save</a>
            </div>
            : ""
        }
        {
          imageDataUri ? <div>Click the 4 corners of the rectangle to square up. Drag to tweak.</div> : ""
        }
        <div id="image-display" onClick={imageDisplayClick}>
          <img id="image" src={imageDataUri} onLoad={imageLoaded} />
          {controlPoints.map((p, i) =>
          (
            <Draggable
              handle=".point"
              onStop={handleDrag}
              defaultPosition={{ x: p[0], y: p[1] }}
              key={`point-${i}`}
              nodeRef={nodeRef}>
              <div className="point" ref={nodeRef} data-index={i}></div>
            </Draggable>
          ))}
        </div>
      </form>
    </main>
  );
}

export default App;
